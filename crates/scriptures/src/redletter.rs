//! The red letters: the words of Christ, located inside each KJV verse.
//!
//! > *"It is the spirit that quickeneth; the flesh profiteth nothing: the words that I
//! > speak unto you, they are spirit, and they are life."* - John 6:63, KJV
//!
//! WHY derive from markup instead of guessing: a red-letter edition is an editorial
//! fact, so the tier-1 source is a red-letter text, not a heuristic over "Jesus said".
//! eBible.org's public-domain KJV (`eng-kjv2006`, USFM) marks every word of Christ with
//! `\wj ... \wj*`. That edition differs from the reader's `kjv.json` in punctuation and
//! spelling here and there, so each verse is aligned word by word (longest common
//! subsequence) and the reader's own words inherit the red flag. The result is a list
//! of byte spans per verse, stored with the sha256 of the text it indexes so a changed
//! `kjv.json` can never silently misplace them.

use crate::bible::Bible;
use crate::canon;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

/// One USFM verse: its words in order, each flagged as Christ's words or narration.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UsfmVerse {
    /// `(lower-cased word, is_words_of_jesus)`.
    pub words: Vec<(String, bool)>,
}

impl UsfmVerse {
    /// True when any word of the verse is marked `\wj`.
    #[must_use]
    pub fn has_red(&self) -> bool {
        self.words.iter().any(|(_, r)| *r)
    }
}

/// The derived red-letter table written to `data/redletter.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RedLetters {
    /// Where the markup came from.
    pub source: String,
    /// sha256 of the source archive.
    pub source_sha256: String,
    /// sha256 of the `kjv.json` the byte spans index.
    pub kjv_sha256: String,
    /// Verses whose USFM carries `\wj`.
    pub usfm_red_verses: usize,
    /// Words marked `\wj` in the USFM.
    pub usfm_red_words: usize,
    /// Words of `kjv.json` painted red after alignment.
    pub aligned_red_words: usize,
    /// OSIS key -> `[start, end)` byte spans into the verse text.
    pub spans: BTreeMap<String, Vec<(usize, usize)>>,
}

impl RedLetters {
    /// Loads the table and refuses it when it indexes a different `kjv.json`.
    ///
    /// # Errors
    /// Fails when unreadable, malformed, or built against other KJV bytes.
    pub fn load(path: &Path, kjv_sha256: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        let me: Self = serde_json::from_str(&raw).map_err(|e| Error::json("redletter.json", e))?;
        if me.kjv_sha256 != kjv_sha256 {
            return Err(Error::Data(format!(
                "redletter.json indexes kjv sha256 {} but data/kjv.json is {kjv_sha256}; rerun fetch-redletter",
                me.kjv_sha256
            )));
        }
        Ok(me)
    }

    /// Spans for a verse key (empty when Christ does not speak in it).
    #[must_use]
    pub fn spans(&self, osis: &str) -> &[(usize, usize)] {
        self.spans.get(osis).map_or(&[], Vec::as_slice)
    }
}

/// Reads every `*.usfm` member of the eBible zip into `(usfm book code, verses)`.
///
/// # Errors
/// Fails when the archive cannot be opened or a member is not UTF-8.
pub fn read_usfm_zip(path: &Path) -> Result<BTreeMap<(usize, usize, usize), UsfmVerse>> {
    let file = std::fs::File::open(path).map_err(|e| Error::io("open", path, e))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::Data(format!("zip {}: {e}", path.display())))?;
    let mut all = BTreeMap::new();
    for i in 0..zip.len() {
        let mut member = zip
            .by_index(i)
            .map_err(|e| Error::Data(format!("zip member {i}: {e}")))?;
        let name = member.name().to_string();
        let is_usfm = Path::new(&name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("usfm"));
        if !is_usfm {
            continue;
        }
        let mut text = String::new();
        member
            .read_to_string(&mut text)
            .map_err(|e| Error::io("read zip member", name.clone(), e))?;
        parse_usfm(&text, &mut all)?;
    }
    Ok(all)
}

/// Parses one USFM book into `(book index, chapter, verse) -> words`.
///
/// Footnotes (`\f`), cross-references (`\x`) and headings are dropped so editorial
/// text can never turn red; `\w word|strong=".."\w*` keeps only the word.
///
/// # Errors
/// Fails when the book code is outside the 66-book canon or a number is malformed.
pub fn parse_usfm(text: &str, out: &mut BTreeMap<(usize, usize, usize), UsfmVerse>) -> Result<()> {
    let mut book: Option<usize> = None;
    let mut chapter = 0usize;
    let mut verse = 0usize;
    let mut state = ScanState::default();
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("\\id ") {
            let code = rest.split_whitespace().next().unwrap_or("");
            book = canon::index_of_usfm(code);
            if book.is_none() {
                return Ok(()); // a book outside the 66 (front matter, glossary): skip it whole
            }
            continue;
        }
        let Some(b) = book else { continue };
        if let Some(rest) = line.strip_prefix("\\c ") {
            chapter = number(rest)?;
            verse = 0;
            state = ScanState::default();
            continue;
        }
        // Headings and titles carry no verse text.
        if [
            "\\s", "\\ms", "\\mr", "\\r ", "\\d ", "\\h ", "\\toc", "\\mt", "\\ide", "\\rem",
        ]
        .iter()
        .any(|m| line.starts_with(m))
        {
            continue;
        }
        let mut rest = line;
        while let Some(pos) = rest.find("\\v ") {
            let (before, from_v) = rest.split_at(pos);
            if verse > 0 {
                scan_words(
                    before,
                    out.entry((b, chapter, verse)).or_default(),
                    &mut state,
                );
            }
            let after = from_v.get(3..).unwrap_or("");
            let num_end = after
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(after.len());
            let (num, tail) = after.split_at(num_end);
            verse = number(num)?;
            rest = tail;
        }
        if verse > 0 && chapter > 0 {
            scan_words(
                rest,
                out.entry((b, chapter, verse)).or_default(),
                &mut state,
            );
        }
    }
    Ok(())
}

fn number(s: &str) -> Result<usize> {
    let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse()
        .map_err(|_| Error::Data(format!("bad USFM number {s:?}")))
}

/// Marker state carried across lines and verses of one chapter, because a saying
/// (`\wj`) or a note may open on one line and close on a later one.
#[derive(Debug, Default)]
struct ScanState {
    wj: u32,
    skip_note: Option<&'static str>,
    in_attr: bool,
}

/// Streams one line fragment, tracking `\wj` depth and skipping notes and attributes.
fn scan_words(fragment: &str, verse: &mut UsfmVerse, state: &mut ScanState) {
    let ScanState {
        wj,
        skip_note,
        in_attr,
    } = state;
    let mut word = String::new();
    let mut chars = fragment.char_indices();
    let flush = |word: &mut String, wj: u32, verse: &mut UsfmVerse| {
        if !word.is_empty() {
            verse.words.push((std::mem::take(word), wj > 0));
        }
    };
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            flush(&mut word, *wj, verse);
            let tail = fragment.get(i + 1..).unwrap_or("");
            let end = tail
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '+' || ch == '*'))
                .unwrap_or(tail.len());
            let marker = tail.get(..end).unwrap_or("");
            for _ in 0..end {
                chars.next();
            }
            let bare = marker.trim_start_matches('+');
            if let Some(close) = *skip_note {
                if bare == close {
                    *skip_note = None;
                }
                continue;
            }
            match bare {
                "wj" => *wj += 1,
                "wj*" => *wj = wj.saturating_sub(1),
                "f" => *skip_note = Some("f*"),
                "x" => *skip_note = Some("x*"),
                "fe" => *skip_note = Some("fe*"),
                "w*" => *in_attr = false,
                _ => {}
            }
            continue;
        }
        if skip_note.is_some() || *in_attr {
            continue;
        }
        if c == '|' {
            flush(&mut word, *wj, verse);
            *in_attr = true;
            continue;
        }
        if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
            for lc in c.to_lowercase() {
                word.push(if lc == '\u{2019}' { '\'' } else { lc });
            }
        } else {
            flush(&mut word, *wj, verse);
        }
    }
    flush(&mut word, *wj, verse);
}

/// A word of the reader's verse: its byte span and normalised form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Start byte.
    pub start: usize,
    /// End byte (exclusive).
    pub end: usize,
    /// Lower-cased, apostrophes normalised.
    pub norm: String,
}

/// Splits a verse into word tokens with byte spans, using the same word rule as USFM.
#[must_use]
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur: Option<(usize, String)> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
            let entry = cur.get_or_insert_with(|| (i, String::new()));
            for lc in c.to_lowercase() {
                entry.1.push(if lc == '\u{2019}' { '\'' } else { lc });
            }
        } else if let Some((start, norm)) = cur.take() {
            out.push(Token {
                start,
                end: i,
                norm,
            });
        }
    }
    if let Some((start, norm)) = cur {
        out.push(Token {
            start,
            end: text.len(),
            norm,
        });
    }
    out
}

/// Aligns USFM words onto the reader's tokens and returns the red flag per token.
///
/// Matched words take the USFM flag; an unmatched word turns red only when the
/// nearest matched words on both sides are red, so a spelling variant inside a
/// saying stays red and narration around it stays black.
#[must_use]
pub fn align(tokens: &[Token], usfm: &[(String, bool)]) -> Vec<bool> {
    let n = tokens.len();
    let m = usfm.len();
    let width = m + 1;
    let mut dp = vec![0u16; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let same = tokens
                .get(i)
                .zip(usfm.get(j))
                .is_some_and(|(t, u)| t.norm == u.0);
            let val = if same {
                dp.get((i + 1) * width + j + 1)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(1)
            } else {
                let down = dp.get((i + 1) * width + j).copied().unwrap_or(0);
                let right = dp.get(i * width + j + 1).copied().unwrap_or(0);
                down.max(right)
            };
            if let Some(cell) = dp.get_mut(i * width + j) {
                *cell = val;
            }
        }
    }
    let mut flags: Vec<Option<bool>> = vec![None; n];
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        let same = tokens
            .get(i)
            .zip(usfm.get(j))
            .is_some_and(|(t, u)| t.norm == u.0);
        if same {
            if let (Some(f), Some(u)) = (flags.get_mut(i), usfm.get(j)) {
                *f = Some(u.1);
            }
            i += 1;
            j += 1;
        } else {
            let down = dp.get((i + 1) * width + j).copied().unwrap_or(0);
            let right = dp.get(i * width + j + 1).copied().unwrap_or(0);
            if down >= right {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    (0..n)
        .map(|k| {
            flags.get(k).copied().flatten().unwrap_or_else(|| {
                let left = flags.get(..k).and_then(|s| s.iter().rev().find_map(|f| *f));
                let right = flags.get(k + 1..).and_then(|s| s.iter().find_map(|f| *f));
                left == Some(true) && right == Some(true)
            })
        })
        .collect()
}

/// Turns per-token red flags into byte spans, carrying closing punctuation that is
/// glued to the last red word (the full stop of a saying belongs to the saying).
#[must_use]
pub fn spans_from_flags(text: &str, tokens: &[Token], flags: &[bool]) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut run: Option<(usize, usize)> = None;
    for (t, red) in tokens.iter().zip(flags) {
        match (run, red) {
            (None, true) => run = Some((t.start, t.end)),
            (Some((s, _)), true) => run = Some((s, t.end)),
            (Some(r), false) => {
                spans.push(r);
                run = None;
            }
            (None, false) => {}
        }
    }
    if let Some(r) = run {
        spans.push(r);
    }
    spans
        .into_iter()
        .map(|(s, e)| {
            let tail = text.get(e..).unwrap_or("");
            let glued: usize = tail
                .chars()
                .take_while(|c| {
                    matches!(
                        c,
                        '.' | ',' | ';' | ':' | '?' | '!' | '\'' | '\u{2019}' | ')'
                    )
                })
                .map(char::len_utf8)
                .sum();
            (s, e + glued)
        })
        .collect()
}

/// The outcome of [`derive`]: spans plus the counts that prove the alignment held.
#[derive(Debug, Default)]
pub struct Derived {
    /// OSIS key -> byte spans.
    pub spans: BTreeMap<String, Vec<(usize, usize)>>,
    /// Verses with `\wj` in the USFM.
    pub usfm_red_verses: usize,
    /// Words with `\wj` in the USFM.
    pub usfm_red_words: usize,
    /// Reader words painted red.
    pub aligned_red_words: usize,
}

/// Builds the full red-letter table for the reader's text.
#[must_use]
pub fn derive(bible: &Bible, usfm: &BTreeMap<(usize, usize, usize), UsfmVerse>) -> Derived {
    let mut spans = BTreeMap::new();
    let (mut red_verses, mut red_words, mut aligned) = (0usize, 0usize, 0usize);
    for (&(b, c, v), uv) in usfm {
        if !uv.has_red() {
            continue;
        }
        red_verses += 1;
        red_words += uv.words.iter().filter(|(_, r)| *r).count();
        let Some(text) = bible.verse_text(b, c, v) else {
            continue;
        };
        let tokens = tokenize(&text);
        let flags = align(&tokens, &uv.words);
        aligned += flags.iter().filter(|f| **f).count();
        let s = spans_from_flags(&text, &tokens, &flags);
        if !s.is_empty() {
            spans.insert(crate::xref::osis_key(b, c, v), s);
        }
    }
    Derived {
        spans,
        usfm_red_verses: red_verses,
        usfm_red_words: red_words,
        aligned_red_words: aligned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOHN_14_6: &str = "\\id JHN\n\\c 14\n\\v 6 \\w Jesus|strong=\"G2424\"\\w* \\w saith|strong=\"G3004\"\\w* unto \\w him|strong=\"G0846\"\\w*, \\wj  \\+w I|strong=\"G1473\"\\+w* \\+w am|strong=\"G1510\"\\+w* \\+w the way|strong=\"G3598\"\\+w*, \\+w the truth|strong=\"G0225\"\\+w*, \\+w and|strong=\"G2532\"\\+w* \\+w the life|strong=\"G2222\"\\+w*: \\+w no man|strong=\"G3762\"\\+w* \\+w cometh|strong=\"G2064\"\\+w* \\+w unto|strong=\"G4314\"\\+w* \\+w the Father|strong=\"G3962\"\\+w*, \\+w but|strong=\"G3361\"\\+w* \\+w by|strong=\"G1223\"\\+w* \\+w me|strong=\"G1700\"\\+w*.\\wj*\n";

    #[test]
    fn john_fourteen_six_is_red_from_i_am_to_the_full_stop() {
        let mut map = BTreeMap::new();
        parse_usfm(JOHN_14_6, &mut map).ok();
        let verse = map.get(&(42, 14, 6)).cloned().unwrap_or_default();
        assert_eq!(verse.words.first(), Some(&("jesus".to_string(), false)));
        assert_eq!(verse.words.iter().filter(|w| w.1).count(), 18);
        let text = "Jesus saith unto him, I am the way, the truth, and the life: no man cometh unto the Father, but by me.";
        let tokens = tokenize(text);
        let flags = align(&tokens, &verse.words);
        let spans = spans_from_flags(text, &tokens, &flags);
        assert_eq!(spans.len(), 1);
        let red = spans.first().and_then(|(s, e)| text.get(*s..*e));
        assert_eq!(
            red,
            Some(
                "I am the way, the truth, and the life: no man cometh unto the Father, but by me."
            )
        );
    }

    #[test]
    fn a_spelling_variant_inside_a_saying_stays_red() {
        let usfm = vec![
            ("verily".to_string(), true),
            ("i".to_string(), true),
            ("say".to_string(), true),
            ("unto".to_string(), true),
            ("you".to_string(), true),
        ];
        let text = "Verily I saye unto you";
        let tokens = tokenize(text);
        assert_eq!(align(&tokens, &usfm), vec![true; 5]);
    }
}
