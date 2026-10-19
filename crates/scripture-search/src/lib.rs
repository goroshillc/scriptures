//! One tested discovery engine for native qualification and the browser.
//! Queries remain on the visitor's device; source text is never synthesized.
//!
//! The same code answers the search page (compiled to WebAssembly) and the builder's
//! native parity checks: exact phrases, all or any of the words, references typed the way
//! people write them ("John 3:16", "Jn 3", "Ps 23"), King James spellings ("show" finds
//! "shew"), typo tolerance that is always reported as such, the words of Christ alone,
//! counts per book, and the exact characters to highlight.

pub mod progress;
pub mod refs;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An admitted catalogue item. Text is the original verse or artwork metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: usize,
    pub kind: String,
    pub title: String,
    pub text: String,
    pub href: String,
    pub book: String,
    pub testament: String,
    pub artist: String,
    pub topics: Vec<String>,
    pub red_letter: bool,
    pub image: Option<String>,
    /// True for a verse inside the passage of an admitted artwork, and for the
    /// artwork itself, so "verses with art" can be filtered.
    #[serde(default)]
    pub art: bool,
    /// The words of Christ in `text`, as `[start, end)` Unicode-scalar ranges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub red: Vec<[usize; 2]>,
}

/// Serializable URL-backed controls. Empty facets mean all values.
///
/// The flags are the page's independent checkboxes, each mirrored in the shareable URL,
/// so they stay plain booleans rather than a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Query {
    pub q: String,
    pub kind: String,
    pub book: String,
    pub testament: String,
    pub artist: String,
    pub topic: String,
    /// Only the words of Christ: verses with red letters, matched within them.
    pub red_letter: bool,
    /// The whole query as one exact phrase (kept for links made before `mode`).
    pub phrase: bool,
    /// `"all"` (default) of the words, `"any"` of them, or `"phrase"`.
    pub mode: String,
    /// Also match near spellings of words that appear nowhere in the text (typos).
    pub fuzzy: bool,
    /// Match only the spelling typed, without King James variants ("show"/"shew").
    pub exact_spelling: bool,
    pub with_art: bool,
    pub sort: String,
    pub offset: usize,
}

/// How the words of a query combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    All,
    Any,
    Phrase,
}

impl Query {
    fn mode(&self) -> Mode {
        match self.mode.as_str() {
            "phrase" => Mode::Phrase,
            "any" => Mode::Any,
            _ if self.phrase => Mode::Phrase,
            _ => Mode::All,
        }
    }
}

/// Where the query's words fall in one result.
///
/// Ranges are `[start, end)` counted in Unicode scalar values of the original
/// strings, so the browser can wrap exactly those characters in `<mark>` without
/// matching anything itself.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Marks {
    pub title: Vec<[usize; 2]>,
    pub text: Vec<[usize; 2]>,
}

/// Matches in one book, for the per-book counts.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BookCount {
    pub book: String,
    pub count: usize,
}

/// A passage the query names, to open directly.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Jump {
    /// Site link to the verse, chapter or book.
    pub href: String,
    /// "John 3:16", "Psalms 23", "John".
    pub label: String,
    /// True when a chapter was named, so the reader can be taken straight there.
    pub passage: bool,
}

/// Bounded page of matching source identities, never generated prose.
#[derive(Debug, Serialize)]
pub struct Results {
    pub ids: Vec<usize>,
    pub total: usize,
    pub offset: usize,
    pub next: Option<usize>,
    /// One entry per id, in the same order.
    pub marks: Vec<Marks>,
    /// One entry per id: true when the result matched only through a near spelling.
    pub fuzzy: Vec<bool>,
    /// Matches per book across every page, in canonical order (book facet ignored).
    pub books: Vec<BookCount>,
    /// King James spellings searched besides the words typed.
    pub variants: Vec<String>,
    /// Near spellings searched for words found nowhere in the text.
    pub near: Vec<String>,
    /// Every word marked in the results, for highlighting the opened chapter.
    pub terms: Vec<String>,
    /// The passage the query names, when it names one.
    pub jump: Option<Jump>,
}

/// Immutable source documents plus a normalized lookup projection.
pub struct Index {
    docs: Vec<Document>,
    searchable: Vec<String>,
    /// Folded words of Christ per document (empty when it has none).
    red_text: Vec<String>,
    /// Every distinct folded word, sorted, for near-spelling lookup.
    vocabulary: Vec<String>,
    /// Folded verse title ("john 3:16") to document.
    titles: HashMap<String, usize>,
}

const fn fold_char(ch: char) -> char {
    match ch {
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'ü' | 'ù' | 'û' => 'u',
        'ö' | 'ô' | 'ó' => 'o',
        'á' | 'à' | 'â' | 'ä' | 'æ' => 'a',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ç' => 'c',
        '\u{2019}' => '\'',
        other => other,
    }
}

fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(fold_char)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The folded characters of `value` with, for each, the index of the original
/// character it came from (a lowercase expansion keeps pointing at its source).
fn fold_mapped(value: &str) -> (Vec<char>, Vec<usize>) {
    let mut folded = Vec::with_capacity(value.len());
    let mut origin = Vec::with_capacity(value.len());
    for (index, ch) in value.chars().enumerate() {
        for lower in ch.to_lowercase() {
            folded.push(if lower.is_whitespace() {
                ' '
            } else {
                fold_char(lower)
            });
            origin.push(index);
        }
    }
    (folded, origin)
}

fn occurrences(haystack: &[char], needle: &[char]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(start, _)| start)
        .collect()
}

fn is_word_char(ch: Option<&char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric())
}

/// The words of a folded string, without punctuation.
fn words_of(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.trim_matches('\''))
        .filter(|w| !w.is_empty())
}

/// King James spellings that mean the same word, each group searched as one.
const VARIANTS: &[&[&str]] = &[
    &["show", "shew"],
    &["shows", "shews", "sheweth", "showeth"],
    &["showed", "shewed"],
    &["showing", "shewing"],
    &["shown", "shewn"],
    &["says", "saith"],
    &["has", "hath"],
    &["does", "doth", "doeth"],
    &["you", "ye", "thee", "thou"],
    &["your", "thy", "thine"],
    &["yours", "thine"],
    &["yourself", "thyself"],
    &["spoke", "spake"],
    &["forever", "for ever"],
    &["today", "to day"],
    &["tomorrow", "to morrow"],
    &["gray", "grey"],
    &["savior", "saviour"],
    &["honor", "honour"],
    &["honored", "honoured"],
    &["neighbor", "neighbour"],
    &["labor", "labour"],
    &["favor", "favour"],
    &["counselor", "counsellor"],
    &["fulfill", "fulfil"],
    &["plow", "plough"],
    &["ax", "axe"],
    &["burned", "burnt"],
];

/// The other spellings searched for `word`: its variant group, and for a modern
/// third-person verb ("gives", "walks") the King James form ("giveth", "walketh").
fn variants_of(word: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for group in VARIANTS {
        if group.contains(&word) {
            out.extend(
                group
                    .iter()
                    .filter(|w| **w != word)
                    .map(|w| (*w).to_string()),
            );
        }
    }
    let alphabetic = word.chars().all(|c| c.is_ascii_lowercase());
    if alphabetic
        && word.len() >= 5
        && word.ends_with('s')
        && !word.ends_with("ss")
        && !word.ends_with("us")
        && !word.ends_with("is")
    {
        let stem = &word[..word.len() - 1];
        let kjv = if stem.ends_with('e') {
            format!("{stem}th")
        } else {
            format!("{stem}eth")
        };
        if !out.contains(&kjv) {
            out.push(kjv);
        }
    }
    out
}

/// Edit distance counting a swap of two neighbouring letters as one edit (optimal string
/// alignment), so "beleive" is one edit from "believe". Gives up, returning `limit + 1`,
/// once the distance must exceed `limit`.
fn distance_within(a: &[char], b: &[char], limit: usize) -> usize {
    if a.len().abs_diff(b.len()) > limit {
        return limit + 1;
    }
    let width = b.len() + 1;
    let mut rows: Vec<Vec<usize>> = Vec::with_capacity(a.len() + 1);
    rows.push((0..width).collect());
    for i in 1..=a.len() {
        let mut row = vec![0usize; width];
        if let Some(first) = row.first_mut() {
            *first = i;
        }
        let mut row_min = i;
        for j in 1..width {
            let (ca, cb) = (a.get(i - 1), b.get(j - 1));
            let up = rows
                .get(i - 1)
                .and_then(|r| r.get(j))
                .copied()
                .unwrap_or(usize::MAX - 1);
            let diag = rows
                .get(i - 1)
                .and_then(|r| r.get(j - 1))
                .copied()
                .unwrap_or(usize::MAX - 1);
            let left = row.get(j - 1).copied().unwrap_or(usize::MAX - 1);
            let mut value = (up + 1).min(left + 1).min(diag + usize::from(ca != cb));
            if i > 1 && j > 1 && ca == b.get(j - 2) && a.get(i - 2) == cb {
                let swap = rows
                    .get(i - 2)
                    .and_then(|r| r.get(j - 2))
                    .copied()
                    .unwrap_or(usize::MAX - 1);
                value = value.min(swap + 1);
            }
            if let Some(slot) = row.get_mut(j) {
                *slot = value;
            }
            row_min = row_min.min(value);
        }
        if row_min > limit {
            return limit + 1;
        }
        rows.push(row);
    }
    rows.last()
        .and_then(|r| r.last())
        .copied()
        .unwrap_or(limit + 1)
}

/// Ranges of `value` that any of `needles` match, sorted and merged.
fn mark_needles(value: &str, needles: &[Vec<char>], phrase: bool) -> Vec<[usize; 2]> {
    let (folded, origin) = fold_mapped(value);
    let mut ranges: Vec<[usize; 2]> = Vec::new();
    for needle in needles {
        for start in occurrences(&folded, needle) {
            let end = start + needle.len();
            // Mark a word where it begins ("love" in "loved"), never inside another
            // word ("the" in "Father"); short words are marked only when whole, so
            // "the" does not light up "them" and "their" across a verse.
            let before = start.checked_sub(1).and_then(|i| folded.get(i));
            let whole_end = !is_word_char(folded.get(end));
            if !phrase && (is_word_char(before) || (needle.len() <= 3 && !whole_end)) {
                continue;
            }
            if let (Some(first), Some(last)) = (origin.get(start), origin.get(end - 1)) {
                ranges.push([*first, *last + 1]);
            }
        }
    }
    ranges.sort_unstable();
    let mut merged: Vec<[usize; 2]> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range[0] <= last[1] => last[1] = last[1].max(range[1]),
            _ => merged.push(range),
        }
    }
    merged
}

/// Ranges of `value` that the query words match, sorted and merged.
///
/// Offsets count original Unicode scalar values. Words of one letter are matched for
/// search but not marked, so a stray "a" never floods a verse with highlights.
#[must_use]
pub fn highlight(value: &str, q: &str, phrase: bool) -> Vec<[usize; 2]> {
    let query = fold(&q.chars().take(256).collect::<String>());
    let needles: Vec<Vec<char>> = if phrase {
        vec![query.chars().collect()]
    } else {
        query
            .split_whitespace()
            .map(|word| word.chars().collect::<Vec<_>>())
            .filter(|word| word.len() > 1)
            .collect()
    };
    mark_needles(value, &needles, phrase)
}

/// How well a matching document fits: an exact reference first, then a title that
/// contains the query, then the whole query as one run of words, then how many of
/// the query's words it holds, then each word standing whole rather than inside a
/// longer one.
fn score(title: &str, text: &str, q: &str, alternatives: &[Vec<String>], exact_ref: bool) -> usize {
    if q.is_empty() {
        return 0;
    }
    let chars: Vec<char> = text.chars().collect();
    let whole = |term: &str| {
        let needle: Vec<char> = term.chars().collect();
        occurrences(&chars, &needle).iter().any(|start| {
            let before = start.checked_sub(1).and_then(|i| chars.get(i));
            !is_word_char(before) && !is_word_char(chars.get(start + needle.len()))
        })
    };
    let held = alternatives
        .iter()
        .filter(|alts| alts.iter().any(|a| text.contains(a.as_str())))
        .count();
    let whole_words = alternatives
        .iter()
        .filter(|alts| alts.iter().any(|a| whole(a)))
        .count();
    let phrase = alternatives.len() > 1 && text.contains(q);
    usize::from(exact_ref) * 100_000
        + usize::from(title.contains(q)) * 10_000
        + usize::from(phrase) * 1_000
        + held * 100
        + whole_words * 10
}

fn matches_facet(selected: &str, actual: &str) -> bool {
    selected.is_empty() || selected == actual
}

/// One query word with what may stand for it.
struct Term {
    /// The word typed and its King James variants.
    exact: Vec<String>,
    /// Near spellings, used only when no exact form occurs anywhere.
    near: Vec<String>,
}

fn strip_fragment(href: &str) -> &str {
    href.split('#').next().unwrap_or(href)
}

impl Index {
    /// Construct a bounded index without accessing a file or network.
    ///
    /// # Errors
    /// Rejects malformed identities, excessive inputs and unsafe navigation URLs.
    pub fn new(docs: Vec<Document>) -> Result<Self, String> {
        if docs.len() > 50_000 {
            return Err("Catalogue exceeds its item limit".into());
        }
        for (id, doc) in docs.iter().enumerate() {
            if doc.id != id || !matches!(doc.kind.as_str(), "verse" | "art") {
                return Err("Invalid catalogue identity or kind".into());
            }
            if !doc.href.starts_with('/')
                || doc.href.starts_with("//")
                || doc.href.contains('\\')
                || doc.href.chars().any(char::is_control)
                || doc.image.as_deref().is_some_and(|url| {
                    !url.starts_with('/') || url.starts_with("//") || url.contains('\\')
                })
            {
                return Err("Catalogue links must stay on this site".into());
            }
            if doc.text.len() > 16_384 || doc.title.len() > 1024 {
                return Err("Catalogue item exceeds its text limit".into());
            }
            let length = doc.text.chars().count();
            if doc.red.iter().any(|[s, e]| s >= e || *e > length) {
                return Err("Catalogue red-letter range is outside its text".into());
            }
        }
        let searchable: Vec<String> = docs
            .iter()
            .map(|doc| {
                fold(&format!(
                    "{} {} {} {}",
                    doc.title, doc.text, doc.book, doc.artist
                ))
            })
            .collect();
        let red_text = docs
            .iter()
            .map(|doc| {
                let chars: Vec<char> = doc.text.chars().collect();
                fold(
                    &doc.red
                        .iter()
                        .map(|[s, e]| chars.get(*s..*e).map(String::from_iter).unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
            .collect();
        let mut vocabulary: Vec<String> = searchable
            .iter()
            .flat_map(|text| words_of(text).map(str::to_string))
            .collect();
        vocabulary.sort_unstable();
        vocabulary.dedup();
        let titles = docs
            .iter()
            .filter(|doc| doc.kind == "verse")
            .map(|doc| (fold(&doc.title), doc.id))
            .collect();
        Ok(Self {
            docs,
            searchable,
            red_text,
            vocabulary,
            titles,
        })
    }

    /// Whether any word of the text is `word` or begins with it.
    fn known(&self, word: &str) -> bool {
        let at = self.vocabulary.partition_point(|w| w.as_str() < word);
        self.vocabulary.get(at).is_some_and(|w| w.starts_with(word))
    }

    /// The closest words of the text to `word`: those at the smallest distance found,
    /// within one edit (two for six letters or more), at most eight.
    fn near(&self, word: &str) -> Vec<String> {
        let typed: Vec<char> = word.chars().collect();
        if typed.iter().filter(|c| c.is_alphabetic()).count() < 4 {
            return Vec::new();
        }
        let limit = if typed.len() >= 6 { 2 } else { 1 };
        let mut found: Vec<(usize, &String)> = self
            .vocabulary
            .iter()
            .filter_map(|candidate| {
                let chars: Vec<char> = candidate.chars().collect();
                let d = distance_within(&typed, &chars, limit);
                (d <= limit).then_some((d, candidate))
            })
            .collect();
        found.sort();
        let best = found.first().map_or(0, |(d, _)| *d);
        found
            .into_iter()
            .take_while(|(d, _)| *d == best)
            .take(8)
            .map(|(_, w)| w.clone())
            .collect()
    }

    /// Whether every word of a (possibly two-word) spelling occurs in the text.
    fn occurs(&self, spelling: &str) -> bool {
        spelling.split(' ').all(|w| {
            let at = self.vocabulary.partition_point(|v| v.as_str() < w);
            self.vocabulary.get(at).is_some_and(|v| v == w)
        })
    }

    fn terms(&self, q: &str, query: &Query) -> Vec<Term> {
        q.split_whitespace()
            .map(|word| {
                let mut exact = vec![word.to_string()];
                if !query.exact_spelling {
                    let bare: String = word.trim_matches(|c: char| !c.is_alphanumeric()).into();
                    for variant in variants_of(&bare) {
                        // Only spellings the King James text actually uses are searched.
                        if !exact.contains(&variant) && self.occurs(&variant) {
                            exact.push(variant);
                        }
                    }
                }
                let bare: String = word.trim_matches(|c: char| !c.is_alphanumeric()).into();
                let near = if query.fuzzy
                    && bare.chars().all(char::is_alphabetic)
                    && !exact
                        .iter()
                        .any(|w| self.known(w.trim_matches(|c: char| !c.is_alphanumeric())))
                {
                    self.near(&bare)
                } else {
                    Vec::new()
                };
                Term { exact, near }
            })
            .collect()
    }

    /// The passage, chapter or book a query names.
    fn jump(&self, q: &str) -> Option<Jump> {
        let reference = refs::parse(q)?;
        let book = refs::BOOKS.get(reference.book)?;
        let Some(chapter) = reference.chapter else {
            // A bare book name opens the book only when it is plainly a name, so a word
            // such as "is" or "so" is never taken for Isaiah or the Song.
            let typed: String = q.chars().filter(|c| c.is_alphanumeric()).collect();
            if typed.chars().filter(char::is_ascii_alphabetic).count() < 3 {
                return None;
            }
            let first = self.titles.get(&fold(&format!("{} 1:1", book.name)))?;
            let chapter_href = strip_fragment(&self.docs.get(*first)?.href);
            let book_href = chapter_href
                .trim_end_matches('/')
                .rsplit_once('/')
                .map(|(head, _)| format!("{head}/"))?;
            return Some(Jump {
                href: book_href,
                label: book.name.to_string(),
                passage: false,
            });
        };
        let verse = reference.verse.unwrap_or(1);
        let id = self
            .titles
            .get(&fold(&format!("{} {chapter}:{verse}", book.name)))?;
        let doc = self.docs.get(*id)?;
        let (href, label) = match (reference.verse, reference.verse_end) {
            (None, _) => (
                strip_fragment(&doc.href).to_string(),
                format!("{} {chapter}", book.name),
            ),
            (Some(v), None) => (doc.href.clone(), format!("{} {chapter}:{v}", book.name)),
            (Some(v), Some(end)) => (
                doc.href.clone(),
                format!("{} {chapter}:{v}\u{2013}{end}", book.name),
            ),
        };
        Some(Jump {
            href,
            label,
            passage: true,
        })
    }

    /// Combine text and facets deterministically, returning at most forty identities.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn search(&self, query: &Query) -> Results {
        let q = fold(&query.q.chars().take(256).collect::<String>());
        let mode = query.mode();
        let terms = if mode == Mode::Phrase {
            Vec::new()
        } else {
            self.terms(&q, query)
        };
        let compact = q.replace(' ', "");
        // Per-book counts in canonical order: documents are stored in that order, so the
        // first match of each book fixes its place (artwork joins its book's count).
        let mut books: Vec<BookCount> = Vec::new();
        let mut found: Vec<(&Document, usize, bool, Vec<String>)> = Vec::new();
        for ((doc, text), red) in self.docs.iter().zip(&self.searchable).zip(&self.red_text) {
            if !matches_facet(&query.kind, &doc.kind)
                || !matches_facet(&query.testament, &doc.testament)
                || !matches_facet(&query.artist, &doc.artist)
                || (!query.topic.is_empty() && !doc.topics.contains(&query.topic))
                || (query.red_letter && !doc.red_letter)
                || (query.with_art && !doc.art)
            {
                continue;
            }
            let haystack = if query.red_letter { red } else { text };
            let title = fold(&doc.title);
            let exact_ref = !compact.is_empty() && title.replace(' ', "") == compact;
            let mut fuzzy = false;
            let mut used: Vec<String> = Vec::new();
            let matched = exact_ref
                || match mode {
                    Mode::Phrase => {
                        let hit = haystack.contains(&q);
                        if hit {
                            used.push(q.clone());
                        }
                        hit
                    }
                    Mode::All | Mode::Any => {
                        let mut held = 0usize;
                        for term in &terms {
                            let exact: Vec<&String> = term
                                .exact
                                .iter()
                                .filter(|w| haystack.contains(w.as_str()))
                                .collect();
                            if exact.is_empty() {
                                let near: Vec<&String> = term
                                    .near
                                    .iter()
                                    .filter(|w| haystack.contains(w.as_str()))
                                    .collect();
                                if !near.is_empty() {
                                    fuzzy = true;
                                    held += 1;
                                    used.extend(near.into_iter().cloned());
                                }
                            } else {
                                held += 1;
                                used.extend(exact.into_iter().cloned());
                            }
                        }
                        if terms.is_empty() {
                            true
                        } else if mode == Mode::All {
                            held == terms.len()
                        } else {
                            held > 0
                        }
                    }
                };
            if !matched {
                continue;
            }
            if let Some(existing) = books.iter_mut().find(|b| b.book == doc.book) {
                existing.count += 1;
            } else {
                books.push(BookCount {
                    book: doc.book.clone(),
                    count: 1,
                });
            }
            if !matches_facet(&query.book, &doc.book) {
                continue;
            }
            let alternatives: Vec<Vec<String>> = terms
                .iter()
                .map(|t| t.exact.iter().chain(&t.near).cloned().collect())
                .collect();
            let rank = score(&title, haystack, &q, &alternatives, exact_ref);
            found.push((doc, rank, fuzzy, used));
        }
        match query.sort.as_str() {
            "title" => found.sort_by(|a, b| a.0.title.cmp(&b.0.title).then(a.0.id.cmp(&b.0.id))),
            "canonical" => found.sort_by_key(|(doc, _, _, _)| doc.id),
            _ => found.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.cmp(&a.1)).then(a.0.id.cmp(&b.0.id))),
        }
        let total = found.len();
        let offset = query.offset.min(total.saturating_sub(1) / 40 * 40);
        let page: Vec<(&Document, bool, Vec<String>)> = found
            .into_iter()
            .skip(offset)
            .take(40)
            .map(|(doc, _, fuzzy, used)| (doc, fuzzy, used))
            .collect();
        let mut marked_terms: Vec<String> = Vec::new();
        let marks = page
            .iter()
            .map(|(doc, _, used)| {
                for word in used {
                    if !marked_terms.contains(word) {
                        marked_terms.push(word.clone());
                    }
                }
                let phrase = mode == Mode::Phrase;
                let needles: Vec<Vec<char>> = if phrase {
                    vec![q.chars().collect()]
                } else {
                    used.iter()
                        .map(|w| w.chars().collect::<Vec<_>>())
                        .filter(|w| w.len() > 1)
                        .collect()
                };
                let mut text = mark_needles(&doc.text, &needles, phrase);
                if query.red_letter {
                    text.retain(|[s, e]| doc.red.iter().any(|[rs, re]| s >= rs && e <= re));
                }
                Marks {
                    title: mark_needles(&doc.title, &needles, phrase),
                    text,
                }
            })
            .collect();
        let variants = terms
            .iter()
            .flat_map(|t| t.exact.iter().skip(1).cloned())
            .collect();
        let near = terms.iter().flat_map(|t| t.near.iter().cloned()).collect();
        Results {
            ids: page.iter().map(|(doc, _, _)| doc.id).collect(),
            total,
            offset,
            next: (offset + 40 < total).then_some(offset + 40),
            marks,
            fuzzy: page.iter().map(|(_, fuzzy, _)| *fuzzy).collect(),
            books,
            variants,
            near,
            terms: marked_terms,
            jump: self.jump(&query.q),
        }
    }
}

/// The chapter (and verse) a typed reference names, as JSON, or `"null"`: the header
/// search box uses it to open a passage at once, before any catalogue is loaded.
#[must_use]
pub fn passage_json(q: &str) -> String {
    let found = refs::parse(q).and_then(|r| {
        let book = refs::BOOKS.get(r.book)?;
        let chapter = r.chapter?;
        let label = match (r.verse, r.verse_end) {
            (None, _) => format!("{} {chapter}", book.name),
            (Some(v), None) => format!("{} {chapter}:{v}", book.name),
            (Some(v), Some(e)) => format!("{} {chapter}:{v}\u{2013}{e}", book.name),
        };
        Some(serde_json::json!({
            "slug": book.slug,
            "chapter": chapter,
            "verse": r.verse,
            "label": label,
        }))
    });
    found.map_or_else(|| "null".to_string(), |v| v.to_string())
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::progress::{self, Event, Store};
    use super::{Index, highlight};
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    thread_local! { static INDEX: RefCell<Option<Index>> = const { RefCell::new(None) }; }

    fn fail(message: &str) -> JsValue {
        JsValue::from_str(message)
    }

    #[wasm_bindgen]
    pub fn initialize(raw: &str) -> Result<(), JsValue> {
        if raw.len() > 32 * 1024 * 1024 {
            return Err(fail("Catalogue too large"));
        }
        let docs = serde_json::from_str(raw).map_err(|_| fail("Invalid catalogue"))?;
        let index = Index::new(docs).map_err(|error| fail(&error))?;
        INDEX.with(|cell| *cell.borrow_mut() = Some(index));
        Ok(())
    }

    /// Highlight ranges for one passage of text, for pages reached from a search.
    /// Needs no catalogue, so a chapter page can mark its own verses.
    #[wasm_bindgen]
    pub fn mark(text: &str, q: &str, phrase: bool) -> Result<String, JsValue> {
        if text.len() > 65_536 || q.len() > 1024 {
            return Err(fail("Text too large"));
        }
        serde_json::to_string(&highlight(text, q, phrase))
            .map_err(|_| fail("Highlight unavailable"))
    }

    #[wasm_bindgen]
    pub fn search(raw: &str) -> Result<String, JsValue> {
        if raw.len() > 4096 {
            return Err(fail("Query too large"));
        }
        let query = serde_json::from_str(raw).map_err(|_| fail("Invalid query"))?;
        INDEX.with(|cell| {
            let borrow = cell.borrow();
            let index = borrow
                .as_ref()
                .ok_or_else(|| fail("Catalogue not loaded"))?;
            serde_json::to_string(&index.search(&query)).map_err(|_| fail("Search unavailable"))
        })
    }

    /// The passage a typed reference names, without the catalogue: `{"slug", "chapter",
    /// "verse", "label"}` or `null` when the text is not a reference to a chapter.
    #[wasm_bindgen]
    pub fn passage(q: &str) -> String {
        super::passage_json(q)
    }

    fn store(raw: &str) -> Result<Store, JsValue> {
        Store::parse(raw).map_err(|e| fail(&e.0))
    }

    fn write(store: &Store) -> Result<String, JsValue> {
        serde_json::to_string(store).map_err(|_| fail("The record could not be written."))
    }

    /// A new device record with one profile.
    #[wasm_bindgen]
    pub fn progress_new(id: &str, name: &str) -> Result<String, JsValue> {
        write(&Store::new(id, name).map_err(|e| fail(&e.0))?)
    }

    /// The record, checked and in canonical form.
    #[wasm_bindgen]
    pub fn progress_check(raw: &str) -> Result<String, JsValue> {
        write(&store(raw)?)
    }

    /// Applies one event: returns `{"store": ..., "credit": ...}`.
    #[wasm_bindgen]
    pub fn progress_apply(
        raw: &str,
        event: &str,
        date: &str,
        seed: u32,
    ) -> Result<String, JsValue> {
        if event.len() > 16_384 {
            return Err(fail("The event is too large."));
        }
        let mut s = store(raw)?;
        let event: Event =
            serde_json::from_str(event).map_err(|_| fail("The event is not valid."))?;
        let credit = s.apply(&event, date, seed).map_err(|e| fail(&e.0))?;
        serde_json::to_string(&serde_json::json!({"store": s, "credit": credit}))
            .map_err(|_| fail("The record could not be written."))
    }

    /// The profile page's view.
    #[wasm_bindgen]
    pub fn progress_view(raw: &str, date: &str) -> Result<String, JsValue> {
        let s = store(raw)?;
        let view = progress::view(&s, date).map_err(|e| fail(&e.0))?;
        serde_json::to_string(&view).map_err(|_| fail("The view could not be written."))
    }

    /// Changes the record's settings or profiles (see [`progress::Change`]).
    #[wasm_bindgen]
    pub fn progress_change(raw: &str, change: &str) -> Result<String, JsValue> {
        let mut s = store(raw)?;
        let change: progress::Change =
            serde_json::from_str(change).map_err(|_| fail("The change is not valid."))?;
        s.change(&change).map_err(|e| fail(&e.0))?;
        write(&s)
    }

    /// Imports one exported profile.
    #[wasm_bindgen]
    pub fn progress_import(raw: &str, file: &str) -> Result<String, JsValue> {
        let mut s = store(raw)?;
        s.import(file).map_err(|e| fail(&e.0))?;
        write(&s)
    }

    /// One profile as an export file.
    #[wasm_bindgen]
    pub fn progress_export(raw: &str, id: &str) -> Result<String, JsValue> {
        progress::export(&store(raw)?, id).map_err(|e| fail(&e.0))
    }

    /// Word positions hidden in a memory check.
    #[wasm_bindgen]
    pub fn progress_blanks(text: &str, seed: u32) -> Result<String, JsValue> {
        if text.len() > 8_192 {
            return Err(fail("Text too large"));
        }
        serde_json::to_string(&progress::blanks(text, seed)).map_err(|_| fail("Unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(id: usize, title: &str, text: &str) -> Document {
        Document {
            id,
            title: title.into(),
            text: text.into(),
            kind: "verse".into(),
            href: "/scriptures/john/3/#v16".into(),
            book: "John".into(),
            testament: "New Testament".into(),
            artist: String::new(),
            topics: vec!["love".into()],
            red_letter: true,
            image: None,
            art: false,
            red: Vec::new(),
        }
    }

    #[test]
    fn references_and_intersecting_filters_find_original_identity() -> Result<(), String> {
        let index = Index::new(vec![
            document(0, "John 3:16", "For God so loved the world"),
            document(1, "John 3:17", "For God sent not his Son"),
        ])?;
        let mut q = Query {
            q: "john3:16".into(),
            ..Query::default()
        };
        assert_eq!(index.search(&q).ids, vec![0]);
        q.q = "GOD world".into();
        q.topic = "love".into();
        q.red_letter = true;
        q.exact_spelling = true;
        assert_eq!(index.search(&q).ids, Vec::<usize>::new(), "no red ranges");
        q.red_letter = false;
        assert_eq!(index.search(&q).ids, vec![0]);
        q.book = "Genesis".into();
        assert_eq!(index.search(&q).total, 0);
        Ok(())
    }

    #[test]
    fn phrase_and_facet_only_queries_are_distinct() -> Result<(), String> {
        let index = Index::new(vec![document(0, "John 3:16", "For God so loved the world")])?;
        assert_eq!(
            index
                .search(&Query {
                    q: "God world".into(),
                    phrase: true,
                    ..Query::default()
                })
                .total,
            0
        );
        assert_eq!(
            index
                .search(&Query {
                    q: "God world".into(),
                    mode: "phrase".into(),
                    ..Query::default()
                })
                .total,
            0
        );
        assert_eq!(
            index
                .search(&Query {
                    topic: "love".into(),
                    ..Query::default()
                })
                .total,
            1
        );
        Ok(())
    }

    #[test]
    fn catalogue_rejects_script_urls_and_duplicate_identities() {
        let mut doc = document(0, "A", "B");
        doc.href = "javascript:alert(1)".into();
        assert!(Index::new(vec![doc]).is_err());
        assert!(Index::new(vec![document(1, "A", "B")]).is_err());
        let mut red = document(0, "A", "B");
        red.red = vec![[0, 9]];
        assert!(Index::new(vec![red]).is_err());
    }

    #[test]
    fn pagination_and_ties_are_stable_and_bounded() -> Result<(), String> {
        let docs = (0..85).map(|id| document(id, "Same", "Original")).collect();
        let index = Index::new(docs)?;
        assert_eq!(index.search(&Query::default()).ids.len(), 40);
        let last = index.search(&Query {
            offset: usize::MAX,
            ..Query::default()
        });
        assert_eq!(last.ids, (80..85).collect::<Vec<_>>());
        assert_eq!(last.next, None);
        Ok(())
    }

    #[test]
    fn highlights_mark_the_original_characters_and_merge_overlaps() {
        let text = "For God so loved the world, that he gave his only begotten Son";
        assert_eq!(
            highlight(text, "LOVED world", false),
            vec![[11, 16], [21, 26]]
        );
        assert_eq!(highlight(text, "love", false), vec![[11, 15]]);
        assert_eq!(highlight(text, "a god", false), vec![[4, 7]]);
        assert_eq!(highlight(text, "loved the world", true), vec![[11, 26]]);
        assert_eq!(highlight("Gustave Doré", "dore", false), vec![[8, 12]]);
        assert_eq!(
            highlight("unto the Father, he loved them", "the", false),
            vec![[5, 8]]
        );
        assert_eq!(highlight(text, "", false), Vec::<[usize; 2]>::new());
    }

    #[test]
    fn whole_words_and_phrases_rank_above_fragments() -> Result<(), String> {
        let index = Index::new(vec![
            document(0, "Psalms 1:1", "the beloved of the world"),
            document(1, "John 3:16", "For God so loved the world"),
            document(2, "John 3:17", "loved, the world"),
        ])?;
        let found = index.search(&Query {
            q: "loved the world".into(),
            ..Query::default()
        });
        assert_eq!(found.ids, vec![1, 2, 0]);
        assert_eq!(found.marks.len(), 3);
        assert_eq!(
            found.marks.first().map(|m| m.text.clone()),
            Some(vec![[11, 16], [17, 20], [21, 26]])
        );
        Ok(())
    }

    #[test]
    fn the_art_facet_keeps_only_verses_and_works_with_art() -> Result<(), String> {
        let mut with_art = document(1, "John 3:17", "For God sent not his Son");
        with_art.art = true;
        let index = Index::new(vec![document(0, "John 3:16", "For God so loved"), with_art])?;
        let found = index.search(&Query {
            with_art: true,
            ..Query::default()
        });
        assert_eq!(found.ids, vec![1]);
        Ok(())
    }

    #[test]
    fn artist_accents_do_not_require_a_special_keyboard() -> Result<(), String> {
        let mut doc = document(0, "A scene", "An engraving");
        doc.kind = "art".into();
        doc.artist = "Gustave Doré".into();
        let index = Index::new(vec![doc])?;
        assert_eq!(
            index
                .search(&Query {
                    q: "dore".into(),
                    ..Query::default()
                })
                .total,
            1
        );
        Ok(())
    }

    fn psalms() -> Result<Index, String> {
        let mut docs = vec![
            document(
                0,
                "Psalms 23:1",
                "The LORD is my shepherd; I shall not want.",
            ),
            document(
                1,
                "Psalms 23:2",
                "He maketh me to lie down in green pastures",
            ),
            document(2, "John 1:1", "In the beginning was the Word"),
            document(
                3,
                "John 14:6",
                "Jesus saith unto him, I am the way, the truth, and the life",
            ),
            document(4, "Psalms 16:11", "Thou wilt shew me the path of life"),
        ];
        for doc in &mut docs {
            doc.book = doc.title.split(' ').next().unwrap_or("").into();
            doc.href = format!(
                "/scriptures/{}/{}/#v{}",
                doc.book.to_lowercase(),
                doc.title.split([' ', ':']).nth(1).unwrap_or("1"),
                doc.title.rsplit(':').next().unwrap_or("1")
            );
            doc.red_letter = false;
        }
        if let Some(doc) = docs.get_mut(3) {
            doc.red_letter = true;
            doc.red = vec![[22, 59]];
        }
        Index::new(docs)
    }

    #[test]
    fn typed_references_jump_to_the_passage() -> Result<(), String> {
        let index = psalms()?;
        let jump = |q: &str| {
            index
                .search(&Query {
                    q: q.into(),
                    ..Query::default()
                })
                .jump
        };
        assert_eq!(
            jump("Ps 23"),
            Some(Jump {
                href: "/scriptures/psalms/23/".into(),
                label: "Psalms 23".into(),
                passage: true
            })
        );
        assert_eq!(
            jump("ps 23:2").map(|j| j.href),
            Some("/scriptures/psalms/23/#v2".into())
        );
        assert_eq!(jump("Jn 1").map(|j| j.label), Some("John 1".into()));
        assert_eq!(
            jump("John").map(|j| (j.href, j.passage)),
            Some(("/scriptures/john/".into(), false))
        );
        assert_eq!(jump("so"), None);
        assert_eq!(jump("shepherd"), None);
        Ok(())
    }

    #[test]
    fn any_all_and_phrase_modes_combine_words_as_named() -> Result<(), String> {
        let index = psalms()?;
        let count = |q: &str, mode: &str| {
            index
                .search(&Query {
                    q: q.into(),
                    mode: mode.into(),
                    ..Query::default()
                })
                .total
        };
        assert_eq!(count("shepherd pastures", "all"), 0);
        assert_eq!(count("shepherd pastures", "any"), 2);
        assert_eq!(count("the way, the truth", "phrase"), 1);
        assert_eq!(count("the truth, the way", "phrase"), 0);
        Ok(())
    }

    #[test]
    fn king_james_spellings_are_found_and_reported() -> Result<(), String> {
        let index = psalms()?;
        let found = index.search(&Query {
            q: "show me".into(),
            ..Query::default()
        });
        assert_eq!(found.ids, vec![4]);
        assert_eq!(found.variants, vec!["shew".to_string()]);
        assert_eq!(
            found.marks.first().map(|m| m.text.clone()),
            Some(vec![[10, 14], [15, 17]])
        );
        let exact = index.search(&Query {
            q: "show me".into(),
            exact_spelling: true,
            ..Query::default()
        });
        assert_eq!(exact.total, 0);
        let says = index.search(&Query {
            q: "says".into(),
            ..Query::default()
        });
        assert_eq!(says.ids, vec![3]);
        assert_eq!(variants_of("gives"), vec!["giveth".to_string()]);
        assert_eq!(variants_of("walks"), vec!["walketh".to_string()]);
        assert_eq!(variants_of("jesus"), Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn typos_match_only_when_asked_and_are_marked_fuzzy() -> Result<(), String> {
        let index = psalms()?;
        let plain = index.search(&Query {
            q: "shepard".into(),
            ..Query::default()
        });
        assert_eq!(plain.total, 0);
        let fuzzy = index.search(&Query {
            q: "shepard".into(),
            fuzzy: true,
            ..Query::default()
        });
        assert_eq!(fuzzy.ids, vec![0]);
        assert_eq!(fuzzy.fuzzy, vec![true]);
        assert_eq!(fuzzy.near, vec!["shepherd".to_string()]);
        assert_eq!(fuzzy.terms, vec!["shepherd".to_string()]);
        // A word that is in the text never pulls in near spellings.
        let known = index.search(&Query {
            q: "life".into(),
            fuzzy: true,
            ..Query::default()
        });
        assert_eq!(known.near, Vec::<String>::new());
        assert!(known.fuzzy.iter().all(|f| !f));
        assert_eq!(
            distance_within(&['a', 'b'], &['a', 'b', 'c', 'd', 'e'], 1),
            2
        );
        Ok(())
    }

    #[test]
    fn red_letter_search_matches_only_the_words_of_christ() -> Result<(), String> {
        let index = psalms()?;
        let red = |q: &str| {
            index.search(&Query {
                q: q.into(),
                red_letter: true,
                ..Query::default()
            })
        };
        // "life" is in John 14:6's red words and in Psalm 16:11, which has none.
        assert_eq!(red("life").ids, vec![3]);
        // "saith" stands before the red words, so it does not match within them.
        assert_eq!(red("saith").total, 0);
        let marks = red("the way").marks;
        assert!(
            marks
                .first()
                .is_some_and(|m| m.text.iter().all(|[s, _]| *s >= 22))
        );
        Ok(())
    }

    #[test]
    fn a_typed_passage_resolves_without_a_catalogue() {
        assert_eq!(
            passage_json("Jn 3:16"),
            r#"{"chapter":3,"label":"John 3:16","slug":"john","verse":16}"#
        );
        assert_eq!(
            passage_json("ps 23"),
            r#"{"chapter":23,"label":"Psalms 23","slug":"psalms","verse":null}"#
        );
        assert_eq!(passage_json("John"), "null");
        assert_eq!(passage_json("love one another"), "null");
    }

    #[test]
    fn counts_are_given_per_book_in_canonical_order() -> Result<(), String> {
        let index = psalms()?;
        let found = index.search(&Query {
            q: "the".into(),
            book: "John".into(),
            ..Query::default()
        });
        assert_eq!(
            found.books,
            vec![
                BookCount {
                    book: "Psalms".into(),
                    count: 2
                },
                BookCount {
                    book: "John".into(),
                    count: 2
                }
            ]
        );
        assert_eq!(
            found.total, 2,
            "the book facet narrows the results, not the counts"
        );
        Ok(())
    }
}
