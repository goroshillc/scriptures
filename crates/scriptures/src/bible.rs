//! The King James text, loaded from `kjv.json` - a port of the original engine's `BOOKS` loader.
//!
//! > *"Heaven and earth shall pass away, but my words shall not pass away."*
//! > - Matthew 24:35, KJV
//!
//! WHY a faithful port rather than a fresh design: the public site must show exactly
//! the text the private reader shows. `clean` reproduces the reader's two regex
//! replacements byte for byte, and the parity test holds the two engines together.

use crate::canon;
use crate::{Error, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RawBook {
    name: String,
    chapters: Vec<Vec<String>>,
}

/// The whole KJV, 66 books in canon order.
#[derive(Debug)]
pub struct Bible {
    books: Vec<RawBook>,
}

impl Bible {
    /// Loads `kjv.json` from disk.
    ///
    /// # Errors
    /// Fails when the file is unreadable, is not the expected JSON, or does not hold
    /// the 66 books in canon order.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        Self::from_json(&raw)
    }

    /// Parses the `kjv.json` document.
    ///
    /// # Errors
    /// Fails on malformed JSON or when the book order disagrees with [`canon::BOOKS`].
    pub fn from_json(raw: &str) -> Result<Self> {
        let books: Vec<RawBook> =
            serde_json::from_str(raw).map_err(|e| Error::json("kjv.json", e))?;
        if books.len() != canon::BOOKS.len() {
            return Err(Error::Data(format!(
                "kjv.json has {} books, expected 66",
                books.len()
            )));
        }
        for (b, info) in books.iter().zip(canon::BOOKS.iter()) {
            if b.name != info.name {
                return Err(Error::Data(format!(
                    "kjv.json book {} where the canon expects {}",
                    b.name, info.name
                )));
            }
        }
        Ok(Self { books })
    }

    /// Book display name, as the reader's `nameOf`.
    #[must_use]
    pub fn book_name(&self, book: usize) -> Option<&str> {
        self.books.get(book).map(|b| b.name.as_str())
    }

    /// Number of chapters in a book (0 when the book does not exist).
    #[must_use]
    pub fn chapter_count(&self, book: usize) -> usize {
        self.books.get(book).map_or(0, |b| b.chapters.len())
    }

    /// Number of verses in a chapter (0 when the chapter does not exist; chapters are 1-based).
    #[must_use]
    pub fn verse_count(&self, book: usize, chapter: usize) -> usize {
        self.chapter(book, chapter).map_or(0, <[String]>::len)
    }

    fn chapter(&self, book: usize, chapter: usize) -> Option<&[String]> {
        let ci = chapter.checked_sub(1)?;
        self.books.get(book)?.chapters.get(ci).map(Vec::as_slice)
    }

    /// The raw verse string as stored (1-based chapter and verse).
    #[must_use]
    pub fn raw_verse(&self, book: usize, chapter: usize, verse: usize) -> Option<&str> {
        let vi = verse.checked_sub(1)?;
        self.chapter(book, chapter)?.get(vi).map(String::as_str)
    }

    /// The reader's `verseText`: the cleaned verse, or `None` when absent or empty.
    #[must_use]
    pub fn verse_text(&self, book: usize, chapter: usize, verse: usize) -> Option<String> {
        self.raw_verse(book, chapter, verse)
            .filter(|t| !t.is_empty())
            .map(clean)
    }

    /// Total verses in the corpus.
    #[must_use]
    pub fn total_verses(&self) -> usize {
        self.books
            .iter()
            .map(|b| b.chapters.iter().map(Vec::len).sum::<usize>())
            .sum()
    }

    /// Total chapters in the corpus.
    #[must_use]
    pub fn total_chapters(&self) -> usize {
        self.books.iter().map(|b| b.chapters.len()).sum()
    }
}

/// Port of the reader's `clean`: `{x}` becomes `*x*`, then `[x]` becomes `x`.
///
/// Mirrors JavaScript's global regex scan: a `{` only opens a group when a `}`
/// follows somewhere later; otherwise it is kept literally.
#[must_use]
pub fn clean(t: &str) -> String {
    let braces = replace_groups(t, '{', '}', "*", "*");
    replace_groups(&braces, '[', ']', "", "")
}

fn replace_groups(t: &str, open: char, close: char, pre: &str, post: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(start) = rest.find(open) {
        let (before, from_open) = rest.split_at(start);
        out.push_str(before);
        let after_open = from_open.get(open.len_utf8()..).unwrap_or("");
        if let Some(end) = after_open.find(close) {
            let (inner, tail) = after_open.split_at(end);
            out.push_str(pre);
            out.push_str(inner);
            out.push_str(post);
            rest = tail.get(close.len_utf8()..).unwrap_or("");
        } else {
            out.push(open);
            rest = after_open;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_turns_braces_into_italic_marks_and_drops_brackets() {
        assert_eq!(clean("the {LORD} [is] good"), "the *LORD* is good");
        assert_eq!(clean("a { b"), "a { b");
        assert_eq!(clean("x [y"), "x [y");
    }
}
