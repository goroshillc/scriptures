//! A private record of a reader's own walk: reading, prayer, memory and study.
//!
//! Everything here runs on the reader's own device. The browser keeps the record in its
//! own storage and hands it to these functions as JSON; nothing is sent anywhere, and
//! nothing compares one reader with another. There are no leaderboards and no sharing.
//!
//! # The curve
//!
//! Levels 1 to 99 follow the Old School `RuneScape` experience curve exactly: the
//! experience for level `L` is `floor(1/4 * sum(n = 1 .. L-1) floor(n + 300 * 2^(n/7)))`,
//! so level 99 needs 13,034,431. That formula grows by `2^(n/7)` and passes `u64` near
//! level 126, so it cannot simply be continued. Above 99 every further level costs a flat
//! [`STEP_AFTER_99`] experience (about a day and a half of steady reading or prayer), so
//! the walk never ends and never becomes impossible: level 9,001 needs 66,446,431.
//!
//! Every value is an integer. The `2^(n/7)` term is evaluated exactly with integer
//! arithmetic (the smallest `y` with `y^7 > 300^7 * 2^n` is compared in a small
//! multi-limb integer), so no floating-point number is involved anywhere.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::refs::BOOKS;

/// Experience for each level beyond 99.
pub const STEP_AFTER_99: u64 = 6_000;
/// Experience at level 99.
pub const XP_AT_99: u64 = 13_034_431;
/// The most experience one skill can hold. It stays far below 2^53 so the record keeps
/// its exact value as JSON in any browser.
pub const MAX_XP: u64 = 1_000_000_000_000_000;
/// Profiles kept on one device.
pub const MAX_PROFILES: usize = 6;
/// Verses kept in the memory list.
pub const MAX_MEMORY: usize = 500;
/// Reader-named prayers kept in the rule.
pub const MAX_PRAYERS: usize = 12;
/// Recent notes kept on the profile page.
pub const MAX_NOTES: usize = 24;

// ---------------------------------------------------------------------------------------
// Exact integer evaluation of the curve
// ---------------------------------------------------------------------------------------

/// A non-negative integer in little-endian 32-bit limbs, just enough for the curve.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn one() -> Self {
        Self(vec![1])
    }

    fn mul_small(&mut self, factor: u32) {
        let mut carry = 0u64;
        for limb in &mut self.0 {
            let product = u64::from(*limb) * u64::from(factor) + carry;
            *limb = u32::try_from(product & 0xffff_ffff).unwrap_or(0);
            carry = product >> 32;
        }
        if carry > 0 {
            self.0.push(u32::try_from(carry).unwrap_or(0));
        }
        self.trim();
    }

    fn shl(&mut self, bits: u32) {
        for _ in 0..bits {
            self.mul_small(2);
        }
    }

    fn trim(&mut self) {
        while self.0.len() > 1 && self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    fn cmp_to(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
}

/// `floor(300 * 2^(n/7))`, exactly: the largest `y` with `y^7 <= 300^7 * 2^n`.
fn floor_300_pow(n: u32) -> u64 {
    let mut target = Big::one();
    for _ in 0..7 {
        target.mul_small(300);
    }
    target.shl(n);
    let (mut low, mut high) = (0u32, 1u32 << 26);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        let mut power = Big::one();
        for _ in 0..7 {
            power.mul_small(mid);
        }
        if power.cmp_to(&target) == std::cmp::Ordering::Greater {
            high = mid;
        } else {
            low = mid;
        }
    }
    u64::from(low)
}

fn table() -> &'static [u64; 100] {
    static TABLE: std::sync::OnceLock<[u64; 100]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out = [0u64; 100];
        let mut points = 0u64;
        for level in 2..=99u32 {
            let n = level - 1;
            points += u64::from(n) + floor_300_pow(n);
            if let Some(slot) = out.get_mut(usize::try_from(level).unwrap_or(0)) {
                *slot = points / 4;
            }
        }
        out
    })
}

/// Experience needed to reach `level` (level 1 needs none).
#[must_use]
pub fn xp_for_level(level: u64) -> u64 {
    if level <= 1 {
        return 0;
    }
    if level <= 99 {
        return usize::try_from(level)
            .ok()
            .and_then(|i| table().get(i).copied())
            .unwrap_or(XP_AT_99);
    }
    XP_AT_99.saturating_add((level - 99).saturating_mul(STEP_AFTER_99))
}

/// The level a total of experience has reached.
#[must_use]
pub fn level_for_xp(xp: u64) -> u64 {
    if xp >= XP_AT_99 {
        return 99 + (xp - XP_AT_99) / STEP_AFTER_99;
    }
    let mut level = 1u64;
    for (index, needed) in table().iter().enumerate().skip(2) {
        if xp >= *needed {
            level = u64::try_from(index).unwrap_or(level);
        } else {
            break;
        }
    }
    level
}

/// `1234567` -> `"1,234,567"`.
#[must_use]
pub fn group(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Progress within the current level in parts per million, and that value written as a
/// CSS percentage with four decimals (`"37.2154%"`), both from integers only.
#[must_use]
pub fn bar(xp: u64) -> (u64, String) {
    let level = level_for_xp(xp);
    let low = xp_for_level(level);
    let high = xp_for_level(level + 1);
    let span = high.saturating_sub(low).max(1);
    let ppm = u64::try_from(u128::from(xp.saturating_sub(low)) * 1_000_000 / u128::from(span))
        .unwrap_or(0)
        .min(1_000_000);
    (ppm, format!("{}.{:04}%", ppm / 10_000, ppm % 10_000))
}

// ---------------------------------------------------------------------------------------
// Calendar days, as integers
// ---------------------------------------------------------------------------------------

/// Days since 1970-01-01 for a `YYYY-MM-DD` civil date (Howard Hinnant's algorithm).
#[must_use]
pub fn day_number(date: &str) -> Option<i64> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year: i64 = date.get(0..4)?.parse().ok()?;
    let month: i64 = date.get(5..7)?.parse().ok()?;
    let day: i64 = date.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_len = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day > month_len {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

// ---------------------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------------------

/// The four skills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skill {
    /// Chapters read through to the end.
    Reading,
    /// The prayer rule.
    Prayer,
    /// Verses learned by heart.
    Memory,
    /// Cross-references followed and plates contemplated.
    Study,
}

impl Skill {
    /// Every skill, in display order.
    pub const ALL: [Self; 4] = [Self::Reading, Self::Prayer, Self::Memory, Self::Study];

    /// The display name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Reading => "Reading",
            Self::Prayer => "Prayer",
            Self::Memory => "Memory",
            Self::Study => "Study",
        }
    }
}

/// Experience per skill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Xp {
    /// Reading.
    pub reading: u64,
    /// Prayer.
    pub prayer: u64,
    /// Memory.
    pub memory: u64,
    /// Study.
    pub study: u64,
}

impl Xp {
    /// The experience of one skill.
    #[must_use]
    pub const fn get(&self, skill: Skill) -> u64 {
        match skill {
            Skill::Reading => self.reading,
            Skill::Prayer => self.prayer,
            Skill::Memory => self.memory,
            Skill::Study => self.study,
        }
    }

    fn add(&mut self, skill: Skill, amount: u64) {
        let slot = match skill {
            Skill::Reading => &mut self.reading,
            Skill::Prayer => &mut self.prayer,
            Skill::Memory => &mut self.memory,
            Skill::Study => &mut self.study,
        };
        *slot = slot.saturating_add(amount).min(MAX_XP);
    }
}

/// What has already been credited today, so nothing is counted twice in a day.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Today {
    /// Day number this record belongs to.
    pub day: i64,
    /// Chapters credited (`"john/3"`).
    pub chapters: Vec<String>,
    /// Cross-reference targets credited.
    pub xrefs: Vec<String>,
    /// Plates credited (slugs).
    pub plates: Vec<String>,
    /// Prayers credited (`"morning"`, `"evening"`, or a reader-named prayer).
    pub prayers: Vec<String>,
    /// Repetitions of the Jesus Prayer counted today.
    pub repetitions: u32,
    /// Verses whose memory check was credited today.
    pub memory: Vec<String>,
}

/// A day-by-day streak, kept as two counts and the last active day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Streak {
    /// Consecutive days ending at `last`.
    pub current: u32,
    /// The longest run.
    pub longest: u32,
    /// The last day anything was recorded.
    pub last: i64,
}

/// One verse kept for memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryVerse {
    /// "John 3:16".
    pub reference: String,
    /// The verse's link on this site.
    pub href: String,
    /// The King James text, as the reader's page showed it.
    pub text: String,
    /// Checks passed.
    #[serde(default)]
    pub passed: u32,
}

/// A quiet note for the profile page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    /// Day number.
    pub day: i64,
    /// What was recorded.
    pub text: String,
}

/// A level reached at one of the milestone levels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    /// The skill.
    pub skill: Skill,
    /// The level reached.
    pub level: u64,
    /// Day number.
    pub day: i64,
}

/// One reader's record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Local identity (`[a-z0-9-]`, 4 to 32 characters).
    pub id: String,
    /// The name the reader chose.
    pub name: String,
    /// Experience per skill.
    #[serde(default)]
    pub xp: Xp,
    /// Chapters read in the current reading of the whole Bible, by book slug.
    #[serde(default)]
    pub books: BTreeMap<String, Vec<u16>>,
    /// Completed readings of all 66 books.
    #[serde(default)]
    pub readings: u32,
    /// The day-by-day streak.
    #[serde(default)]
    pub streak: Streak,
    /// What was credited today.
    #[serde(default)]
    pub today: Today,
    /// Verses kept for memory.
    #[serde(default)]
    pub memory: Vec<MemoryVerse>,
    /// Reader-named prayers in the rule (names only; no text is supplied by the site).
    #[serde(default)]
    pub prayers: Vec<String>,
    /// Recent notes, newest last.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Milestones reached, oldest first.
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

/// Everything kept on one device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    /// Format version (1).
    pub version: u32,
    /// The profile in use.
    pub active: String,
    /// Keep a private record at all. Off means nothing is recorded or shown.
    #[serde(default = "yes")]
    pub record: bool,
    /// Show a faint one-line note at the page edge when something is recorded.
    #[serde(default = "yes")]
    pub notes: bool,
    /// The profiles.
    pub profiles: Vec<Profile>,
}

const fn yes() -> bool {
    true
}

/// Why a record was refused.
#[derive(Debug, PartialEq, Eq)]
pub struct Invalid(pub String);

fn invalid(reason: &str) -> Invalid {
    Invalid(reason.to_string())
}

fn clean_name(name: &str) -> Result<String, Invalid> {
    let trimmed: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = trimmed.chars().count();
    if count == 0 || count > 40 || trimmed.chars().any(char::is_control) {
        return Err(invalid("A name is 1 to 40 letters."));
    }
    if trimmed.chars().any(|c| matches!(c, '<' | '>' | '"' | '`')) {
        return Err(invalid("A name cannot contain < > \" or `."));
    }
    Ok(trimmed)
}

fn id_ok(id: &str) -> bool {
    (4..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn text_ok(value: &str, max: usize) -> bool {
    value.chars().count() <= max && !value.chars().any(|c| c.is_control() && c != '\n')
}

fn book_chapters(slug: &str) -> Option<u16> {
    BOOKS.iter().find(|b| b.slug == slug).map(|b| b.chapters)
}

impl Profile {
    /// A new, empty record.
    ///
    /// # Errors
    /// Refuses an empty or over-long name, or an id outside `[a-z0-9-]{4,32}`.
    pub fn new(id: &str, name: &str) -> Result<Self, Invalid> {
        if !id_ok(id) {
            return Err(invalid("The profile id is not valid."));
        }
        Ok(Self {
            id: id.to_string(),
            name: clean_name(name)?,
            xp: Xp::default(),
            books: BTreeMap::new(),
            readings: 0,
            streak: Streak::default(),
            today: Today::default(),
            memory: Vec::new(),
            prayers: Vec::new(),
            notes: Vec::new(),
            milestones: Vec::new(),
        })
    }

    /// Checks every field and puts the record in its canonical form: chapters sorted,
    /// deduplicated and inside their book, experience within bounds, lists bounded.
    ///
    /// # Errors
    /// Refuses a record that is not plainly one of ours.
    pub fn normalize(&mut self) -> Result<(), Invalid> {
        if !id_ok(&self.id) {
            return Err(invalid("The profile id is not valid."));
        }
        self.name = clean_name(&self.name)?;
        for skill in Skill::ALL {
            if self.xp.get(skill) > MAX_XP {
                return Err(invalid("An experience total is out of range."));
            }
        }
        let mut books = BTreeMap::new();
        for (slug, chapters) in &self.books {
            let Some(count) = book_chapters(slug) else {
                return Err(invalid("A book in the record is not one of the 66."));
            };
            let mut list: Vec<u16> = chapters.clone();
            list.sort_unstable();
            list.dedup();
            if list.iter().any(|c| *c == 0 || *c > count) {
                return Err(invalid("A chapter in the record is outside its book."));
            }
            if !list.is_empty() {
                books.insert(slug.clone(), list);
            }
        }
        self.books = books;
        if self.memory.len() > MAX_MEMORY
            || self.memory.iter().any(|m| {
                !text_ok(&m.reference, 40)
                    || !text_ok(&m.text, 2_000)
                    || !m.href.starts_with('/')
                    || m.href.starts_with("//")
                    || !text_ok(&m.href, 200)
            })
        {
            return Err(invalid("A memory verse in the record is not valid."));
        }
        if self.prayers.len() > MAX_PRAYERS {
            return Err(invalid("Too many prayers in the rule."));
        }
        let mut names = Vec::new();
        for prayer in &self.prayers {
            let name = clean_name(prayer)?;
            if !names.contains(&name) {
                names.push(name);
            }
        }
        self.prayers = names;
        let today = &self.today;
        if today.chapters.len() > 2_000
            || today.xrefs.len() > 2_000
            || today.plates.len() > 500
            || today.prayers.len() > 64
            || today.memory.len() > MAX_MEMORY
        {
            return Err(invalid("The day's record is too long."));
        }
        if self.notes.len() > MAX_NOTES {
            let excess = self.notes.len() - MAX_NOTES;
            self.notes.drain(..excess);
        }
        if self.notes.iter().any(|n| !text_ok(&n.text, 300)) || self.milestones.len() > 10_000 {
            return Err(invalid("A note in the record is not valid."));
        }
        Ok(())
    }

    fn roll_day(&mut self, day: i64) {
        if self.today.day != day {
            self.today = Today {
                day,
                ..Today::default()
            };
        }
    }

    fn mark_active(&mut self, day: i64) {
        let s = &mut self.streak;
        if s.last == day {
            return;
        }
        s.current = if s.last == day - 1 {
            s.current.saturating_add(1)
        } else {
            1
        };
        s.last = day;
        s.longest = s.longest.max(s.current);
    }

    fn note(&mut self, day: i64, text: String) {
        self.notes.push(Note { day, text });
        if self.notes.len() > MAX_NOTES {
            self.notes.remove(0);
        }
    }

    /// Adds experience, recording the streak, a note, and any milestone reached.
    fn gain(&mut self, skill: Skill, amount: u64, day: i64, what: &str) -> Credit {
        let before = level_for_xp(self.xp.get(skill));
        self.xp.add(skill, amount);
        let after = level_for_xp(self.xp.get(skill));
        self.mark_active(day);
        self.note(
            day,
            format!(
                "{what} \u{b7} {} {} experience",
                group(amount),
                skill.label()
            ),
        );
        let mut reached = None;
        if after > before {
            self.note(day, format!("{} level {}", skill.label(), group(after)));
            for level in (before + 1)..=after {
                if is_milestone(level) {
                    self.milestones.push(Milestone { skill, level, day });
                    reached = Some(level);
                }
            }
        }
        Credit {
            skill: Some(skill),
            xp: amount,
            level: after,
            level_up: after > before,
            milestone: reached,
            text: format!("{what} \u{b7} {} experience", group(amount)),
        }
    }
}

/// Levels that carry a red-letter verse on the profile page: 10, 50, 92, 99, then
/// every hundredth level.
#[must_use]
pub const fn is_milestone(level: u64) -> bool {
    matches!(level, 10 | 50 | 92 | 99) || (level >= 100 && level.is_multiple_of(100))
}

/// What one event earned.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Credit {
    /// The skill credited, if any.
    pub skill: Option<Skill>,
    /// Experience earned (0 when nothing was credited).
    pub xp: u64,
    /// The skill's level after the event.
    pub level: u64,
    /// Whether a level was reached.
    pub level_up: bool,
    /// A milestone level reached, if any.
    pub milestone: Option<u64>,
    /// A plain one-line description.
    pub text: String,
}

/// Something the reader did.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// A chapter page was open and visible for `visible_ms`, and its last verse was
    /// reached on screen.
    Chapter {
        /// Book slug.
        book: String,
        /// Chapter number.
        chapter: u16,
        /// Verses in the chapter.
        verses: u16,
        /// Words in the chapter.
        words: u32,
        /// Milliseconds the page was visible.
        visible_ms: u64,
        /// The end of the chapter was reached.
        reached_end: bool,
    },
    /// A cross-reference was followed.
    Xref {
        /// Target (`"john/3#v16"`).
        target: String,
    },
    /// An art plate was in view.
    Plate {
        /// The work's slug.
        slug: String,
        /// Milliseconds it was visible.
        visible_ms: u64,
    },
    /// Morning or evening prayers, or a reader-named prayer, were said.
    Prayer {
        /// `"morning"`, `"evening"`, or a reader-named prayer.
        rule: String,
    },
    /// Repetitions of the Jesus Prayer.
    Repetitions {
        /// How many.
        count: u32,
    },
    /// A verse was kept for memory.
    Keep {
        /// Reference.
        reference: String,
        /// Link.
        href: String,
        /// Text.
        text: String,
    },
    /// A verse was let go from the memory list.
    Release {
        /// Reference.
        reference: String,
    },
    /// A memory check was answered.
    Check {
        /// Reference.
        reference: String,
        /// The words written into the blanks, in order.
        answers: Vec<String>,
    },
    /// A prayer name was added to the rule.
    AddPrayer {
        /// The name.
        name: String,
    },
    /// A prayer name was removed from the rule.
    RemovePrayer {
        /// The name.
        name: String,
    },
}

/// Reading experience for a chapter: 40 per verse and 200 for the chapter.
#[must_use]
pub fn chapter_xp(verses: u16) -> u64 {
    200 + 40 * u64::from(verses)
}

/// The least visible time that counts as reading a chapter of `words` words: the time to
/// read it at 450 words a minute, never less than 20 seconds nor more than 15 minutes.
#[must_use]
pub fn reading_time_ms(words: u32) -> u64 {
    (u64::from(words) * 60_000 / 450).clamp(20_000, 900_000)
}

/// Experience for each repetition of the Jesus Prayer.
pub const REPETITION_XP: u64 = 10;
/// Repetitions credited in one day.
pub const REPETITIONS_PER_DAY: u32 = 3_000;
/// Experience for morning or for evening prayers.
pub const RULE_XP: u64 = 1_500;
/// Experience for a reader-named prayer.
pub const NAMED_PRAYER_XP: u64 = 300;
/// Experience for a cross-reference followed.
pub const XREF_XP: u64 = 100;
/// Cross-references credited in one day.
pub const XREFS_PER_DAY: usize = 50;
/// Experience for a plate contemplated.
pub const PLATE_XP: u64 = 250;
/// Time a plate must be in view.
pub const PLATE_MS: u64 = 8_000;
/// Memory experience for each word of a verse said back correctly.
pub const MEMORY_WORD_XP: u64 = 50;

fn nothing(text: &str) -> Credit {
    Credit {
        text: text.to_string(),
        ..Credit::default()
    }
}

fn fold_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The words hidden in a memory check, as 0-based word positions.
///
/// Every third word of three letters or more is hidden, starting at a position chosen by
/// `seed`, and never fewer than one (words are counted by whitespace).
#[must_use]
pub fn blanks(text: &str, seed: u32) -> Vec<usize> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let candidates: Vec<usize> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| fold_word(w).chars().count() >= 3)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let start = usize::try_from(seed % 3).unwrap_or(0);
    let mut chosen: Vec<usize> = candidates.iter().copied().skip(start).step_by(3).collect();
    if chosen.is_empty() {
        chosen.extend(candidates.first());
    }
    chosen
}

/// Whether the answers fill the blanks correctly (case and punctuation ignored).
#[must_use]
pub fn check_answers(text: &str, seed: u32, answers: &[String]) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    let hidden = blanks(text, seed);
    hidden.len() == answers.len()
        && !hidden.is_empty()
        && hidden.iter().zip(answers).all(|(index, answer)| {
            words
                .get(*index)
                .is_some_and(|word| fold_word(word) == fold_word(answer))
        })
}

fn chapter_key(book: &str, chapter: u16) -> String {
    format!("{book}/{chapter}")
}

fn plain(value: &str, max: usize) -> Result<(), Invalid> {
    if value.is_empty() || !text_ok(value, max) {
        return Err(invalid("The event is not valid."));
    }
    Ok(())
}

impl Profile {
    /// Applies one event on `day`, returning what it earned. Each thing is credited at
    /// most once a day, so the record cannot be inflated by repetition or clicking.
    ///
    /// # Errors
    /// Refuses a malformed event.
    #[allow(clippy::too_many_lines)]
    pub fn apply(&mut self, event: &Event, day: i64, seed: u32) -> Result<Credit, Invalid> {
        self.roll_day(day);
        match event {
            Event::Chapter {
                book,
                chapter,
                verses,
                words,
                visible_ms,
                reached_end,
            } => {
                let Some(count) = book_chapters(book) else {
                    return Err(invalid("Unknown book."));
                };
                if *chapter == 0 || *chapter > count || *verses == 0 || *verses > 176 {
                    return Err(invalid("Unknown chapter."));
                }
                let key = chapter_key(book, *chapter);
                if !*reached_end || *visible_ms < reading_time_ms(*words) {
                    return Ok(nothing("Not yet read to the end."));
                }
                if self.today.chapters.contains(&key) {
                    return Ok(nothing("Already recorded today."));
                }
                self.today.chapters.push(key);
                let list = self.books.entry(book.clone()).or_default();
                if !list.contains(chapter) {
                    list.push(*chapter);
                    list.sort_unstable();
                }
                let name = BOOKS
                    .iter()
                    .find(|b| b.slug == *book)
                    .map_or(book.as_str(), |b| b.name);
                let mut credit = self.gain(
                    Skill::Reading,
                    chapter_xp(*verses),
                    day,
                    &format!("{name} {chapter} read"),
                );
                if self.whole_bible_read() {
                    self.readings = self.readings.saturating_add(1);
                    self.books.clear();
                    let n = self.readings;
                    self.note(
                        day,
                        format!(
                            "The whole Bible read through, {} time{}",
                            group(u64::from(n)),
                            if n == 1 { "" } else { "s" }
                        ),
                    );
                    credit.text.push_str(" \u{b7} the whole Bible read through");
                }
                Ok(credit)
            }
            Event::Xref { target } => {
                plain(target, 120)?;
                if self.today.xrefs.contains(target) {
                    return Ok(nothing("Already recorded today."));
                }
                if self.today.xrefs.len() >= XREFS_PER_DAY {
                    return Ok(nothing("Enough study recorded for today."));
                }
                self.today.xrefs.push(target.clone());
                Ok(self.gain(Skill::Study, XREF_XP, day, "Cross-reference followed"))
            }
            Event::Plate { slug, visible_ms } => {
                plain(slug, 120)?;
                if *visible_ms < PLATE_MS {
                    return Ok(nothing("Not yet in view long enough."));
                }
                if self.today.plates.contains(slug) {
                    return Ok(nothing("Already recorded today."));
                }
                self.today.plates.push(slug.clone());
                Ok(self.gain(Skill::Study, PLATE_XP, day, "Plate contemplated"))
            }
            Event::Prayer { rule } => {
                let (xp, label) = match rule.as_str() {
                    "morning" => (RULE_XP, "Morning prayers".to_string()),
                    "evening" => (RULE_XP, "Evening prayers".to_string()),
                    other => {
                        let name = clean_name(other)?;
                        if !self.prayers.contains(&name) {
                            return Err(invalid("That prayer is not in the rule."));
                        }
                        (NAMED_PRAYER_XP, name)
                    }
                };
                if self.today.prayers.contains(rule) {
                    return Ok(nothing("Already recorded today."));
                }
                self.today.prayers.push(rule.clone());
                Ok(self.gain(Skill::Prayer, xp, day, &label))
            }
            Event::Repetitions { count } => {
                if *count == 0 || *count > 10_000 {
                    return Err(invalid("The count is not valid."));
                }
                let room = REPETITIONS_PER_DAY.saturating_sub(self.today.repetitions);
                let credited = (*count).min(room);
                self.today.repetitions = self.today.repetitions.saturating_add(credited);
                if credited == 0 {
                    return Ok(nothing("The day's count is complete."));
                }
                Ok(self.gain(
                    Skill::Prayer,
                    u64::from(credited) * REPETITION_XP,
                    day,
                    &format!(
                        "The Jesus Prayer, {} time{}",
                        group(u64::from(credited)),
                        if credited == 1 { "" } else { "s" }
                    ),
                ))
            }
            Event::Keep {
                reference,
                href,
                text,
            } => {
                plain(reference, 40)?;
                plain(text, 2_000)?;
                if !href.starts_with('/') || href.starts_with("//") || !text_ok(href, 200) {
                    return Err(invalid("The link is not valid."));
                }
                if self.memory.iter().any(|m| m.reference == *reference) {
                    return Ok(nothing("Already kept."));
                }
                if self.memory.len() >= MAX_MEMORY {
                    return Ok(nothing("The memory list is full."));
                }
                self.memory.push(MemoryVerse {
                    reference: reference.clone(),
                    href: href.clone(),
                    text: text.clone(),
                    passed: 0,
                });
                Ok(nothing(&format!("{reference} kept for memory.")))
            }
            Event::Release { reference } => {
                self.memory.retain(|m| m.reference != *reference);
                Ok(nothing(&format!("{reference} let go.")))
            }
            Event::Check { reference, answers } => {
                if answers.len() > 400 || answers.iter().any(|a| !text_ok(a, 60)) {
                    return Err(invalid("The answers are not valid."));
                }
                let Some(verse) = self.memory.iter().find(|m| m.reference == *reference) else {
                    return Err(invalid("That verse is not kept for memory."));
                };
                let text = verse.text.clone();
                if !check_answers(&text, seed, answers) {
                    return Ok(nothing("Not yet; read it again and try once more."));
                }
                if let Some(verse) = self.memory.iter_mut().find(|m| m.reference == *reference) {
                    verse.passed = verse.passed.saturating_add(1);
                }
                if self.today.memory.contains(reference) {
                    return Ok(nothing("Said back correctly; already recorded today."));
                }
                self.today.memory.push(reference.clone());
                let words = u64::try_from(text.split_whitespace().count().min(100)).unwrap_or(0);
                Ok(self.gain(
                    Skill::Memory,
                    words * MEMORY_WORD_XP,
                    day,
                    &format!("{reference} said from memory"),
                ))
            }
            Event::AddPrayer { name } => {
                let name = clean_name(name)?;
                if matches!(name.as_str(), "morning" | "evening") {
                    return Err(invalid("That name is reserved."));
                }
                if self.prayers.contains(&name) {
                    return Ok(nothing("Already in the rule."));
                }
                if self.prayers.len() >= MAX_PRAYERS {
                    return Ok(nothing("The rule is full."));
                }
                self.prayers.push(name);
                Ok(nothing("Added to the rule."))
            }
            Event::RemovePrayer { name } => {
                self.prayers.retain(|p| p != name);
                Ok(nothing("Removed from the rule."))
            }
        }
    }

    fn whole_bible_read(&self) -> bool {
        BOOKS.iter().all(|book| {
            self.books
                .get(book.slug)
                .is_some_and(|list| list.len() == usize::from(book.chapters))
        })
    }
}

impl Store {
    /// A device record with one new profile.
    ///
    /// # Errors
    /// Refuses an invalid id or name.
    pub fn new(id: &str, name: &str) -> Result<Self, Invalid> {
        Ok(Self {
            version: 1,
            active: id.to_string(),
            record: true,
            notes: true,
            profiles: vec![Profile::new(id, name)?],
        })
    }

    /// Parses and checks a stored record.
    ///
    /// # Errors
    /// Refuses anything that is not a valid record of this format.
    pub fn parse(raw: &str) -> Result<Self, Invalid> {
        if raw.len() > 4 * 1024 * 1024 {
            return Err(invalid("The record is too large."));
        }
        let mut store: Self =
            serde_json::from_str(raw).map_err(|_| invalid("The record could not be read."))?;
        store.normalize()?;
        Ok(store)
    }

    /// Checks every profile and the store's own fields.
    ///
    /// # Errors
    /// Refuses an invalid record.
    pub fn normalize(&mut self) -> Result<(), Invalid> {
        if self.version != 1 {
            return Err(invalid("This record is from another version."));
        }
        if self.profiles.is_empty() || self.profiles.len() > MAX_PROFILES {
            return Err(invalid("A record holds 1 to 6 profiles."));
        }
        let mut ids = Vec::new();
        for profile in &mut self.profiles {
            profile.normalize()?;
            if ids.contains(&profile.id) {
                return Err(invalid("Two profiles share one id."));
            }
            ids.push(profile.id.clone());
        }
        if !ids.contains(&self.active) {
            self.active = ids.first().cloned().unwrap_or_default();
        }
        Ok(())
    }

    /// The profile in use.
    #[must_use]
    pub fn current(&self) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == self.active)
    }

    fn current_mut(&mut self) -> Option<&mut Profile> {
        let active = self.active.clone();
        self.profiles.iter_mut().find(|p| p.id == active)
    }

    /// Applies an event to the profile in use (nothing when recording is off).
    ///
    /// # Errors
    /// Refuses a malformed event or date.
    pub fn apply(&mut self, event: &Event, date: &str, seed: u32) -> Result<Credit, Invalid> {
        let day = day_number(date).ok_or_else(|| invalid("The date is not valid."))?;
        if !self.record {
            return Ok(nothing("Recording is off."));
        }
        let profile = self
            .current_mut()
            .ok_or_else(|| invalid("No profile is in use."))?;
        profile.apply(event, day, seed)
    }

    /// Adds a profile and makes it current.
    ///
    /// # Errors
    /// Refuses a seventh profile, a duplicate id, or an invalid name.
    pub fn add(&mut self, id: &str, name: &str) -> Result<(), Invalid> {
        if self.profiles.len() >= MAX_PROFILES {
            return Err(invalid("This device already keeps six profiles."));
        }
        if self.profiles.iter().any(|p| p.id == id) {
            return Err(invalid("That profile id is in use."));
        }
        self.profiles.push(Profile::new(id, name)?);
        self.active = id.to_string();
        Ok(())
    }

    /// Imports one exported profile, replacing a profile with the same id.
    ///
    /// # Errors
    /// Refuses an invalid file, or a seventh profile.
    pub fn import(&mut self, raw: &str) -> Result<String, Invalid> {
        if raw.len() > 4 * 1024 * 1024 {
            return Err(invalid("The file is too large."));
        }
        let file: Export =
            serde_json::from_str(raw).map_err(|_| invalid("The file is not a profile export."))?;
        if file.format != EXPORT_FORMAT {
            return Err(invalid("The file is not a profile export."));
        }
        let mut profile = file.profile;
        profile.normalize()?;
        let id = profile.id.clone();
        if let Some(slot) = self.profiles.iter_mut().find(|p| p.id == id) {
            *slot = profile;
        } else if self.profiles.len() >= MAX_PROFILES {
            return Err(invalid("This device already keeps six profiles."));
        } else {
            self.profiles.push(profile);
        }
        self.active.clone_from(&id);
        Ok(id)
    }
}

/// A change to the device record's profiles or settings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// Switch to another profile.
    Use {
        /// Its id.
        id: String,
    },
    /// Add a profile and switch to it.
    Add {
        /// New id.
        id: String,
        /// Its name.
        name: String,
    },
    /// Rename a profile.
    Rename {
        /// Its id.
        id: String,
        /// The new name.
        name: String,
    },
    /// Remove a profile (never the last one; the page clears the whole record instead).
    Remove {
        /// Its id.
        id: String,
    },
    /// Turn recording on or off.
    Record {
        /// On.
        on: bool,
    },
    /// Turn the faint edge notes on or off.
    Notes {
        /// On.
        on: bool,
    },
}

impl Store {
    /// Applies one change.
    ///
    /// # Errors
    /// Refuses an unknown id, an invalid name, a seventh profile, or removing the last.
    pub fn change(&mut self, change: &Change) -> Result<(), Invalid> {
        match change {
            Change::Use { id } => {
                if !self.profiles.iter().any(|p| p.id == *id) {
                    return Err(invalid("No such profile."));
                }
                self.active.clone_from(id);
            }
            Change::Add { id, name } => self.add(id, name)?,
            Change::Rename { id, name } => {
                let name = clean_name(name)?;
                let profile = self
                    .profiles
                    .iter_mut()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| invalid("No such profile."))?;
                profile.name = name;
            }
            Change::Remove { id } => {
                if self.profiles.len() <= 1 {
                    return Err(invalid("The last profile stays; clear the record instead."));
                }
                let before = self.profiles.len();
                self.profiles.retain(|p| p.id != *id);
                if self.profiles.len() == before {
                    return Err(invalid("No such profile."));
                }
                if self.active == *id {
                    self.active = self
                        .profiles
                        .first()
                        .map(|p| p.id.clone())
                        .unwrap_or_default();
                }
            }
            Change::Record { on } => self.record = *on,
            Change::Notes { on } => self.notes = *on,
        }
        Ok(())
    }
}

/// The marker every export carries.
pub const EXPORT_FORMAT: &str = "goroshi-scriptures-profile-1";

/// One profile as a file the reader keeps.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    /// [`EXPORT_FORMAT`].
    pub format: String,
    /// The profile.
    pub profile: Profile,
}

/// Writes one profile as a pretty JSON export.
///
/// # Errors
/// Refuses an unknown profile id.
pub fn export(store: &Store, id: &str) -> Result<String, Invalid> {
    let profile = store
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| invalid("No such profile."))?
        .clone();
    serde_json::to_string_pretty(&Export {
        format: EXPORT_FORMAT.to_string(),
        profile,
    })
    .map_err(|_| invalid("The profile could not be written."))
}

// ---------------------------------------------------------------------------------------
// The profile page view
// ---------------------------------------------------------------------------------------

/// One skill as the profile page shows it.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SkillView {
    /// `"reading"`.
    pub key: Skill,
    /// `"Reading"`.
    pub label: &'static str,
    /// Level.
    pub level: u64,
    /// Experience, grouped (`"1,234"`).
    pub xp: String,
    /// Experience still needed for the next level, grouped.
    pub to_next: String,
    /// Experience at the next level, grouped.
    pub next_at: String,
    /// Progress through this level in parts per million.
    pub ppm: u64,
    /// The same as a CSS percentage.
    pub percent: String,
}

/// One book on the books map.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BookView {
    /// Name.
    pub name: &'static str,
    /// Slug.
    pub slug: &'static str,
    /// Chapters read in this reading.
    pub read: usize,
    /// Chapters in the book.
    pub chapters: u16,
    /// Every chapter read.
    pub complete: bool,
}

/// Everything the profile page shows for the profile in use.
#[derive(Debug, Serialize)]
pub struct View {
    /// Profiles on this device: (id, name, initial).
    pub profiles: Vec<(String, String, String)>,
    /// The profile in use.
    pub active: String,
    /// Its name.
    pub name: String,
    /// Its initial.
    pub initial: String,
    /// Recording on.
    pub record: bool,
    /// Edge notes on.
    pub notes_on: bool,
    /// Sum of the four levels.
    pub total_level: u64,
    /// Sum of the four totals, grouped.
    pub total_xp: String,
    /// The skills.
    pub skills: Vec<SkillView>,
    /// Current streak (0 when the last active day is before yesterday).
    pub streak: u32,
    /// Longest streak.
    pub longest: u32,
    /// Completed readings of the whole Bible.
    pub readings: u32,
    /// Chapters read in the current reading.
    pub chapters_read: usize,
    /// The books map.
    pub books: Vec<BookView>,
    /// Verses kept for memory.
    pub memory: Vec<MemoryVerse>,
    /// Reader-named prayers.
    pub prayers: Vec<String>,
    /// Prayers recorded today.
    pub prayed_today: Vec<String>,
    /// Repetitions counted today.
    pub repetitions_today: u32,
    /// Recent notes, newest first, with their day numbers.
    pub notes: Vec<Note>,
    /// Milestones, newest first.
    pub milestones: Vec<Milestone>,
}

/// The profile page's view of the profile in use on `date`.
///
/// # Errors
/// Refuses a malformed date or a record with no profile.
pub fn view(store: &Store, date: &str) -> Result<View, Invalid> {
    let day = day_number(date).ok_or_else(|| invalid("The date is not valid."))?;
    let profile = store
        .current()
        .ok_or_else(|| invalid("No profile is in use."))?;
    let skills: Vec<SkillView> = Skill::ALL
        .iter()
        .map(|skill| {
            let xp = profile.xp.get(*skill);
            let level = level_for_xp(xp);
            let next = xp_for_level(level + 1);
            let (ppm, percent) = bar(xp);
            SkillView {
                key: *skill,
                label: skill.label(),
                level,
                xp: group(xp),
                to_next: group(next.saturating_sub(xp)),
                next_at: group(next),
                ppm,
                percent,
            }
        })
        .collect();
    let total: u64 = Skill::ALL.iter().map(|s| profile.xp.get(*s)).sum();
    let books: Vec<BookView> = BOOKS
        .iter()
        .map(|book| {
            let read = profile.books.get(book.slug).map_or(0, Vec::len);
            BookView {
                name: book.name,
                slug: book.slug,
                read,
                chapters: book.chapters,
                complete: read == usize::from(book.chapters),
            }
        })
        .collect();
    let initial = profile
        .name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    let today = if profile.today.day == day {
        profile.today.clone()
    } else {
        Today::default()
    };
    Ok(View {
        profiles: store
            .profiles
            .iter()
            .map(|p| {
                let initial = p
                    .name
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>())
                    .unwrap_or_default();
                (p.id.clone(), p.name.clone(), initial)
            })
            .collect(),
        active: profile.id.clone(),
        name: profile.name.clone(),
        initial,
        record: store.record,
        notes_on: store.notes,
        total_level: skills.iter().map(|s| s.level).sum(),
        total_xp: group(total),
        skills,
        streak: if profile.streak.last >= day - 1 {
            profile.streak.current
        } else {
            0
        },
        longest: profile.streak.longest,
        readings: profile.readings,
        chapters_read: profile.books.values().map(Vec::len).sum(),
        books,
        memory: profile.memory.clone(),
        prayers: profile.prayers.clone(),
        prayed_today: today.prayers,
        repetitions_today: today.repetitions,
        notes: profile.notes.iter().rev().cloned().collect(),
        milestones: profile.milestones.iter().rev().cloned().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_matches_the_formula_at_levels_2_50_92_and_99() {
        // floor(1/4 * sum floor(n + 300 * 2^(n/7))), evaluated exactly.
        assert_eq!(xp_for_level(1), 0);
        assert_eq!(xp_for_level(2), 83);
        assert_eq!(xp_for_level(50), 101_333);
        assert_eq!(xp_for_level(92), 6_517_253);
        assert_eq!(xp_for_level(99), 13_034_431);
        assert_eq!(xp_for_level(99), XP_AT_99);
    }

    #[test]
    fn the_exact_power_term_is_right_where_two_to_the_n_over_seven_is_whole() {
        // n = 7k makes 2^(n/7) a whole power of two, so the term is exact.
        assert_eq!(floor_300_pow(0), 300);
        assert_eq!(floor_300_pow(7), 600);
        assert_eq!(floor_300_pow(14), 1_200);
        assert_eq!(floor_300_pow(98), 300 * 16_384);
        // 300 * 2^(1/7) = 331.2...
        assert_eq!(floor_300_pow(1), 331);
    }

    #[test]
    fn levels_continue_without_a_cap_past_99_in_integers() {
        assert_eq!(xp_for_level(100), XP_AT_99 + STEP_AFTER_99);
        assert_eq!(xp_for_level(9_001), 66_446_431);
        assert_eq!(level_for_xp(66_446_431), 9_001);
        assert_eq!(level_for_xp(66_446_430), 9_000);
        assert_eq!(
            level_for_xp(MAX_XP),
            99 + (MAX_XP - XP_AT_99) / STEP_AFTER_99
        );
        assert_eq!(xp_for_level(u64::MAX), u64::MAX);
        for level in 1..=200 {
            assert_eq!(level_for_xp(xp_for_level(level)), level, "level {level}");
            if level > 1 {
                assert_eq!(level_for_xp(xp_for_level(level) - 1), level - 1);
            }
        }
    }

    #[test]
    fn the_bar_is_exact_to_the_experience() {
        assert_eq!(bar(0), (0, "0.0000%".to_string()));
        // Level 1 spans 0..83; 41 is 493,975 parts per million.
        assert_eq!(bar(41), (493_975, "49.3975%".to_string()));
        assert_eq!(bar(83).0, 0);
        assert_eq!(group(13_034_431), "13,034,431");
        assert_eq!(group(7), "7");
    }

    #[test]
    fn day_numbers_are_civil_and_refuse_impossible_dates() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-10-19"), Some(20_745));
        assert_eq!(day_number("2024-02-29"), Some(19_782));
        assert_eq!(day_number("2026-02-29"), None);
        assert_eq!(day_number("2026-13-01"), None);
        assert_eq!(day_number("19-10-2026"), None);
    }

    fn store() -> Store {
        Store::new("reader-1", "Timothy").unwrap_or_else(|e| panic!("{}", e.0))
    }

    fn read(store: &mut Store, book: &str, chapter: u16, date: &str) -> Credit {
        store
            .apply(
                &Event::Chapter {
                    book: book.into(),
                    chapter,
                    verses: 20,
                    words: 600,
                    visible_ms: 120_000,
                    reached_end: true,
                },
                date,
                0,
            )
            .unwrap_or_else(|e| panic!("{}", e.0))
    }

    #[test]
    fn a_chapter_counts_only_after_its_time_and_its_end_and_once_a_day() {
        let mut s = store();
        let quick = s
            .apply(
                &Event::Chapter {
                    book: "john".into(),
                    chapter: 3,
                    verses: 36,
                    words: 900,
                    visible_ms: 30_000,
                    reached_end: true,
                },
                "2026-10-19",
                0,
            )
            .unwrap_or_default();
        assert_eq!(quick.xp, 0, "900 words need 120 seconds");
        let unread = s
            .apply(
                &Event::Chapter {
                    book: "john".into(),
                    chapter: 3,
                    verses: 36,
                    words: 900,
                    visible_ms: 600_000,
                    reached_end: false,
                },
                "2026-10-19",
                0,
            )
            .unwrap_or_default();
        assert_eq!(unread.xp, 0, "the end was never reached");
        assert_eq!(read(&mut s, "john", 3, "2026-10-19").xp, 1_000);
        assert_eq!(read(&mut s, "john", 3, "2026-10-19").xp, 0);
        assert_eq!(read(&mut s, "john", 3, "2026-10-20").xp, 1_000);
        assert_eq!(s.current().map(|p| p.xp.reading), Some(2_000));
        assert_eq!(s.current().map(|p| p.streak.current), Some(2));
        assert!(
            s.apply(
                &Event::Chapter {
                    book: "john".into(),
                    chapter: 22,
                    verses: 1,
                    words: 1,
                    visible_ms: 1,
                    reached_end: true,
                },
                "2026-10-20",
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn reading_all_sixty_six_books_counts_one_reading_and_starts_the_map_afresh() {
        let mut s = store();
        for book in &BOOKS {
            for chapter in 1..=book.chapters {
                read(&mut s, book.slug, chapter, "2026-10-19");
            }
        }
        let p = s.current().cloned().unwrap_or_else(|| panic!("profile"));
        assert_eq!(p.readings, 1);
        assert!(p.books.is_empty());
        // Next time through: Genesis 1 starts the second reading's map.
        read(&mut s, "genesis", 1, "2026-10-20");
        let p = s.current().cloned().unwrap_or_else(|| panic!("profile"));
        assert_eq!(p.books.get("genesis"), Some(&vec![1]));
        assert_eq!(p.readings, 1);
    }

    #[test]
    fn prayer_counts_once_a_day_and_repetitions_have_a_daily_measure() {
        let mut s = store();
        let morning = Event::Prayer {
            rule: "morning".into(),
        };
        assert_eq!(
            s.apply(&morning, "2026-10-19", 0).map(|c| c.xp),
            Ok(RULE_XP)
        );
        assert_eq!(s.apply(&morning, "2026-10-19", 0).map(|c| c.xp), Ok(0));
        let reps = Event::Repetitions { count: 2_500 };
        assert_eq!(s.apply(&reps, "2026-10-19", 0).map(|c| c.xp), Ok(25_000));
        assert_eq!(s.apply(&reps, "2026-10-19", 0).map(|c| c.xp), Ok(5_000));
        assert_eq!(s.apply(&reps, "2026-10-19", 0).map(|c| c.xp), Ok(0));
        let named = Event::Prayer {
            rule: "For my family".into(),
        };
        assert!(s.apply(&named, "2026-10-19", 0).is_err());
        assert!(
            s.apply(
                &Event::AddPrayer {
                    name: "For my family".into()
                },
                "2026-10-19",
                0
            )
            .is_ok()
        );
        assert_eq!(
            s.apply(&named, "2026-10-19", 0).map(|c| c.xp),
            Ok(NAMED_PRAYER_XP)
        );
    }

    #[test]
    fn study_credits_each_reference_and_plate_once_a_day() {
        let mut s = store();
        let x = Event::Xref {
            target: "john/3#v16".into(),
        };
        assert_eq!(s.apply(&x, "2026-10-19", 0).map(|c| c.xp), Ok(XREF_XP));
        assert_eq!(s.apply(&x, "2026-10-19", 0).map(|c| c.xp), Ok(0));
        let glance = Event::Plate {
            slug: "the-creation-of-light".into(),
            visible_ms: 2_000,
        };
        assert_eq!(s.apply(&glance, "2026-10-19", 0).map(|c| c.xp), Ok(0));
        let gaze = Event::Plate {
            slug: "the-creation-of-light".into(),
            visible_ms: 9_000,
        };
        assert_eq!(s.apply(&gaze, "2026-10-19", 0).map(|c| c.xp), Ok(PLATE_XP));
    }

    #[test]
    fn a_memory_check_passes_only_with_the_right_words() {
        let mut s = store();
        let text = "Jesus wept.";
        s.apply(
            &Event::Keep {
                reference: "John 11:35".into(),
                href: "/scriptures/john/11/#v35".into(),
                text: text.into(),
            },
            "2026-10-19",
            0,
        )
        .unwrap_or_default();
        // "Jesus" and "wept" are candidates; seed 0 hides every third from the first.
        assert_eq!(blanks(text, 0), vec![0]);
        let wrong = Event::Check {
            reference: "John 11:35".into(),
            answers: vec!["Peter".into()],
        };
        assert_eq!(s.apply(&wrong, "2026-10-19", 0).map(|c| c.xp), Ok(0));
        let right = Event::Check {
            reference: "John 11:35".into(),
            answers: vec!["jesus".into()],
        };
        assert_eq!(
            s.apply(&right, "2026-10-19", 0).map(|c| c.xp),
            Ok(2 * MEMORY_WORD_XP)
        );
        assert_eq!(
            s.current().map(|p| p.memory.first().map(|m| m.passed)),
            Some(Some(1))
        );
    }

    #[test]
    fn milestones_are_kept_at_10_50_92_99_and_every_hundredth_level() {
        assert!(is_milestone(10) && is_milestone(50) && is_milestone(92) && is_milestone(99));
        assert!(is_milestone(100) && is_milestone(9_000) && !is_milestone(101));
        let mut p = Profile::new("abcd", "A").unwrap_or_else(|e| panic!("{}", e.0));
        let credit = p.gain(Skill::Prayer, xp_for_level(10), 1, "Prayer");
        assert_eq!(credit.level, 10);
        assert_eq!(credit.milestone, Some(10));
        assert_eq!(p.milestones.len(), 1);
    }

    #[test]
    fn a_profile_exports_and_imports_unchanged_and_bad_files_are_refused() {
        let mut s = store();
        read(&mut s, "john", 3, "2026-10-19");
        let file = export(&s, "reader-1").unwrap_or_default();
        let mut other = Store::new("reader-2", "Other").unwrap_or_else(|e| panic!("{}", e.0));
        assert_eq!(other.import(&file), Ok("reader-1".to_string()));
        assert_eq!(other.profiles.len(), 2);
        assert_eq!(other.current(), s.current());
        // Re-importing replaces rather than duplicates.
        assert_eq!(other.import(&file), Ok("reader-1".to_string()));
        assert_eq!(other.profiles.len(), 2);
        assert!(other.import("{}").is_err());
        assert!(
            other
                .import(&file.replace("john", "gospel-of-thomas"))
                .is_err()
        );
        assert!(
            other
                .import(&file.replace("\"reading\": 1000", "\"reading\": 1000000000000000001"))
                .is_err()
        );
        let round = serde_json::to_string(&other).unwrap_or_default();
        assert_eq!(Store::parse(&round).map(|s| s.profiles.len()), Ok(2));
    }

    #[test]
    fn the_view_shows_levels_streak_and_the_books_map() {
        let mut s = store();
        read(&mut s, "john", 3, "2026-10-18");
        let v = view(&s, "2026-10-19").unwrap_or_else(|e| panic!("{}", e.0));
        assert_eq!(v.books.len(), 66);
        assert_eq!(v.streak, 1);
        assert_eq!(v.chapters_read, 1);
        assert_eq!(v.total_level, 1 + 1 + 1 + level_for_xp(1_000));
        let later = view(&s, "2026-10-25").unwrap_or_else(|e| panic!("{}", e.0));
        assert_eq!(later.streak, 0, "a rest is not counted against anyone");
        assert_eq!(later.longest, 1);
    }

    #[test]
    fn profiles_are_switched_renamed_and_removed_but_never_the_last() {
        let mut s = store();
        assert!(
            s.change(&Change::Add {
                id: "reader-2".into(),
                name: "Lydia".into()
            })
            .is_ok()
        );
        assert_eq!(s.active, "reader-2");
        assert!(
            s.change(&Change::Use {
                id: "nobody".into()
            })
            .is_err()
        );
        assert!(
            s.change(&Change::Rename {
                id: "reader-2".into(),
                name: "  Lydia   T ".into()
            })
            .is_ok()
        );
        assert_eq!(s.current().map(|p| p.name.as_str()), Some("Lydia T"));
        assert!(
            s.change(&Change::Remove {
                id: "reader-2".into()
            })
            .is_ok()
        );
        assert_eq!(s.active, "reader-1");
        assert!(
            s.change(&Change::Remove {
                id: "reader-1".into()
            })
            .is_err()
        );
        for n in 2..=MAX_PROFILES {
            assert!(s.add(&format!("reader-{n}"), "R").is_ok());
        }
        assert!(s.add("reader-99", "R").is_err(), "six profiles at most");
        assert!(Profile::new("abcd", "<script>").is_err());
    }

    #[test]
    fn recording_off_records_nothing() {
        let mut s = store();
        s.record = false;
        assert_eq!(read(&mut s, "john", 3, "2026-10-19").xp, 0);
        assert_eq!(s.current().map(|p| p.xp.reading), Some(0));
    }
}
