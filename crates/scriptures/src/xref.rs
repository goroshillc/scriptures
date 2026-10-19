//! The cross-reference apparatus - a line-for-line port of the original engine.
//!
//! > *"Jesus saith unto them, Did ye never read in the scriptures, The stone which the
//! > builders rejected, the same is become the head of the corner: this is the Lord's
//! > doing, and it is marvellous in our eyes?"* - Matthew 21:42, KJV
//!
//! 326,308 links from the Treasury of Scripture Knowledge via openbible.info (CC BY),
//! keyed by OSIS reference. `parse_one`, `expand_range`, `human` and `resolve` keep the
//! original's exact behaviour, including its edge cases (reversed ranges yield only the
//! start verse, spans are capped at 400 verses and 5,000 steps), because the parity
//! test compares this output with the original engine over the whole corpus.

use crate::bible::Bible;
use crate::canon;
use crate::{Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Range expansion cap, as in the original (`CAP = 400`).
pub const RANGE_CAP: usize = 400;
const STEP_CAP: usize = 5000;

/// A parsed `Book.chapter.verse` location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    /// Canon index of the book.
    pub book: usize,
    /// Chapter number as written (may not exist).
    pub chapter: usize,
    /// Verse number as written (may not exist).
    pub verse: usize,
}

/// One resolved cross-reference: the reader's `{ osis, ref, text }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resolved {
    /// The raw OSIS reference or range (`"1John.4.9-1John.4.10"`).
    pub osis: String,
    /// Human form (`"1 John 4:9-10"`).
    #[serde(rename = "ref")]
    pub reference: String,
    /// Every verse of the range, cleaned and joined by a space.
    pub text: String,
}

/// A verse with its resolved apparatus: the reader's `getVerse` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerseFull {
    /// OSIS key.
    pub osis: String,
    /// Human reference.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Cleaned verse text.
    pub text: String,
    /// Resolved cross-references with non-empty text, in source order.
    #[serde(rename = "crossRefs")]
    pub cross_refs: Vec<Resolved>,
}

/// The cross-reference table, keyed by OSIS verse.
#[derive(Debug, Default)]
pub struct Xrefs {
    map: HashMap<String, Vec<String>>,
}

impl Xrefs {
    /// Loads `xrefs.json`.
    ///
    /// # Errors
    /// Fails when the file is unreadable or not a `{ osis: [refs] }` object.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        let map: HashMap<String, Vec<String>> =
            serde_json::from_str(&raw).map_err(|e| Error::json("xrefs.json", e))?;
        Ok(Self { map })
    }

    /// The raw reference list for a verse key (empty when none).
    #[must_use]
    pub fn refs(&self, osis: &str) -> &[String] {
        self.map.get(osis).map_or(&[], Vec::as_slice)
    }

    /// Number of verses that carry at least one reference.
    #[must_use]
    pub fn keyed_verses(&self) -> usize {
        self.map.len()
    }

    /// Total number of reference links.
    #[must_use]
    pub fn total_links(&self) -> usize {
        self.map.values().map(Vec::len).sum()
    }
}

/// Port of `parseOne`: `^(\w+)\.(\d+)\.(\d+)$` with a known OSIS book code.
#[must_use]
pub fn parse_one(s: &str) -> Option<Loc> {
    let mut parts = s.split('.');
    let code = parts.next()?;
    let ch = parts.next()?;
    let v = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let word = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    if !word(code) || !digits(ch) || !digits(v) {
        return None;
    }
    let book = canon::index_of_osis(code)?;
    Some(Loc {
        book,
        chapter: ch.parse().ok()?,
        verse: v.parse().ok()?,
    })
}

fn split_range(s: &str) -> (&str, Option<&str>) {
    let mut it = s.split('-');
    let first = it.next().unwrap_or("");
    (first, it.next())
}

fn range_end(from: Loc, b_raw: &str) -> Option<Loc> {
    if b_raw.contains('.') {
        parse_one(b_raw)
    } else {
        let code = canon::book(from.book)?.osis;
        parse_one(&format!("{code}.{}.{b_raw}", from.chapter))
    }
}

/// Port of `expandRange`: every verse of a range, walking across chapter and book ends.
#[must_use]
pub fn expand_range(bible: &Bible, s: &str) -> Vec<Loc> {
    let (first, second) = split_range(s);
    let Some(from) = parse_one(first) else {
        return Vec::new();
    };
    let Some(b_raw) = second.filter(|b| !b.is_empty()) else {
        return vec![from];
    };
    let Some(to) = range_end(from, b_raw) else {
        return vec![from];
    };
    let beyond = |bi: usize, ch: usize, v: usize| {
        bi > to.book || (bi == to.book && (ch > to.chapter || (ch == to.chapter && v > to.verse)))
    };
    if beyond(from.book, from.chapter, from.verse) {
        return vec![from];
    }
    let mut out = Vec::new();
    let (mut bi, mut ch, mut v) = (from.book, from.chapter, from.verse);
    let mut iter = 0usize;
    while !beyond(bi, ch, v) && out.len() < RANGE_CAP {
        let more = iter < STEP_CAP;
        iter += 1;
        if !more {
            break;
        }
        if canon::book(bi).is_none() {
            break;
        }
        let vc = bible.verse_count(bi, ch);
        if vc > 0 && v <= vc {
            out.push(Loc {
                book: bi,
                chapter: ch,
                verse: v,
            });
        }
        v += 1;
        if v > vc {
            v = 1;
            ch += 1;
            if ch > bible.chapter_count(bi) {
                ch = 1;
                bi += 1;
            }
        }
    }
    if out.is_empty() { vec![from] } else { out }
}

fn name_of(bible: &Bible, book: usize) -> String {
    bible.book_name(book).map_or_else(
        || canon::book(book).map_or(String::new(), |b| b.osis.to_string()),
        str::to_string,
    )
}

/// Port of `human`: `"John 3:16"`, `"John 3:16-18"`, `"John 3:16-4:2"`, or `"John 3:16…"`.
#[must_use]
pub fn human(bible: &Bible, s: &str) -> String {
    let (first, second) = split_range(s);
    let Some(from) = parse_one(first) else {
        return s.to_string();
    };
    let name = name_of(bible, from.book);
    let Some(b_raw) = second.filter(|b| !b.is_empty()) else {
        return format!("{name} {}:{}", from.chapter, from.verse);
    };
    match range_end(from, b_raw) {
        Some(to) if to.book == from.book && to.chapter == from.chapter => {
            format!("{name} {}:{}-{}", from.chapter, from.verse, to.verse)
        }
        Some(to) if to.book == from.book => {
            format!(
                "{name} {}:{}-{}:{}",
                from.chapter, from.verse, to.chapter, to.verse
            )
        }
        _ => format!("{name} {}:{}\u{2026}", from.chapter, from.verse),
    }
}

/// Port of `resolveRef`: the range's verse texts joined by a space.
#[must_use]
pub fn resolve(bible: &Bible, s: &str) -> Resolved {
    let text = expand_range(bible, s)
        .iter()
        .filter_map(|p| bible.verse_text(p.book, p.chapter, p.verse))
        .collect::<Vec<_>>()
        .join(" ");
    Resolved {
        osis: s.to_string(),
        reference: human(bible, s),
        text,
    }
}

/// Port of `getVerse`: the verse with every cross-reference resolved to its text.
#[must_use]
pub fn get_verse(bible: &Bible, xrefs: &Xrefs, osis: &str) -> Option<VerseFull> {
    let p = parse_one(osis)?;
    let text = bible.verse_text(p.book, p.chapter, p.verse)?;
    let cross_refs = xrefs
        .refs(osis)
        .iter()
        .map(|r| resolve(bible, r))
        .filter(|x| !x.text.is_empty())
        .collect();
    Some(VerseFull {
        osis: osis.to_string(),
        reference: human(bible, osis),
        text,
        cross_refs,
    })
}

/// The OSIS key of a verse (`"John.3.16"`).
#[must_use]
pub fn osis_key(book: usize, chapter: usize, verse: usize) -> String {
    let code = canon::book(book).map_or("", |b| b.osis);
    format!("{code}.{chapter}.{verse}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_accepts_only_known_codes_and_three_parts() {
        assert_eq!(
            parse_one("John.3.16"),
            Some(Loc {
                book: 42,
                chapter: 3,
                verse: 16
            })
        );
        assert_eq!(parse_one("Jn.3.16"), None);
        assert_eq!(parse_one("John.3"), None);
        assert_eq!(parse_one("John.3.16.1"), None);
        assert_eq!(parse_one("John.3.x"), None);
    }
}
