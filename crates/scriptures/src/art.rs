//! The master art: which public-domain work illustrates which passage, and the
//! licence record read for each file from the Wikimedia Commons API.
//!
//! > *"Then spake Jesus again unto them, saying, I am the light of the world: he that
//! > followeth me shall not walk in darkness, but shall have the light of life."*
//! > - John 8:12, KJV
//!
//! WHY two files: `data/art_plan.tsv` is the human curation (file, scene, passage,
//! note), editable in any text editor; `data/art_manifest.json` is what the Commons API
//! actually reported for each admitted file (artist, date, licence strings verbatim,
//! sha1, source URL) plus the local derivatives. The plan is intent; the manifest is
//! evidence. Nothing reaches the site without a manifest row.

use crate::xref::{self, Loc};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One curated row of `art_plan.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    /// Commons file title, with the `File:` prefix.
    pub file: String,
    /// Artist key: `dore`, `durer` or `titian`.
    pub artist: String,
    /// Scene title shown on the site.
    pub scene: String,
    /// OSIS passage (`"Gen.1.1-Gen.1.5"`), or `None` for books outside the KJV's 66.
    pub passage: Option<String>,
    /// Human reference for passages outside the 66 books (`"Tobit 6:1-18"`).
    pub other_ref: Option<String>,
    /// Curation note: why the mapping differs from the Commons caption, if it does.
    pub note: Option<String>,
}

/// Parses `art_plan.tsv` (tab-separated: file, artist, scene, passage, `other_ref`, note;
/// `-` for an empty field; `#` starts a comment line).
///
/// # Errors
/// Fails on a row without the six fields.
pub fn parse_plan(raw: &str) -> Result<Vec<PlanRow>> {
    let opt = |s: &str| {
        let t = s.trim();
        if t.is_empty() || t == "-" {
            None
        } else {
            Some(t.to_string())
        }
    };
    raw.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(n, l)| {
            let f: Vec<&str> = l.split('\t').collect();
            match f.as_slice() {
                [file, artist, scene, passage, other, note] => Ok(PlanRow {
                    file: file.trim().to_string(),
                    artist: artist.trim().to_string(),
                    scene: scene.trim().to_string(),
                    passage: opt(passage),
                    other_ref: opt(other),
                    note: opt(note),
                }),
                _ => Err(Error::Data(format!(
                    "art_plan.tsv line {}: expected 6 tab-separated fields, got {}",
                    n + 1,
                    f.len()
                ))),
            }
        })
        .collect()
}

/// A local derivative of an admitted file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variant {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// File name under the site's `/art/img/`.
    pub file: String,
}

/// The downloaded master a work's display images are resized from.
///
/// WHY recorded: Commons gives the sha1 and size of its original upload, but what this
/// site serves is resized from a Commons rendition of it. The sha256, byte count and
/// pixels of the exact bytes received prove which file every derivative came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MasterRecord {
    /// The URL the bytes were downloaded from (Commons original or its rendition).
    pub url: String,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Byte count.
    pub bytes: u64,
    /// sha256 of the bytes, lowercase hex.
    pub sha256: String,
}

/// One admitted artwork: the curated plan joined with the Commons evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artwork {
    /// URL slug (`"the-creation-of-light"`).
    pub slug: String,
    /// Commons file title.
    pub commons_file: String,
    /// Commons description page.
    pub commons_url: String,
    /// Artist key.
    pub artist_key: String,
    /// Artist as Commons states it (markup stripped).
    pub artist: String,
    /// Artist life dates for the credit line.
    pub artist_dates: String,
    /// Date of the work as Commons states it (markup stripped).
    pub date: String,
    /// Scene title.
    pub scene: String,
    /// Medium for the credit line ("Wood engraving", "Woodcut", "Oil on canvas").
    pub medium: String,
    /// Commons `ImageDescription`, markup stripped.
    pub commons_description: String,
    /// Commons `LicenseShortName`, verbatim.
    pub licence_short: String,
    /// Commons `License` code, verbatim (`pd`, `cc0`).
    pub licence_code: String,
    /// Commons `UsageTerms`, verbatim.
    pub usage_terms: String,
    /// Commons `LicenseUrl` when given.
    pub licence_url: Option<String>,
    /// Commons categories (used to state the Public Domain Mark truthfully).
    pub categories: Vec<String>,
    /// Commons sha1 of the original upload.
    pub sha1: String,
    /// Original width and height on Commons.
    pub original_size: (u32, u32),
    /// Original file size in bytes as the Commons API reports it.
    #[serde(default)]
    pub original_bytes: u64,
    /// The file actually downloaded and resized here: pixels, bytes and sha256.
    #[serde(default)]
    pub master: Option<MasterRecord>,
    /// Passage in OSIS, when inside the 66 books.
    pub passage: Option<String>,
    /// Reference outside the 66 books.
    pub other_ref: Option<String>,
    /// Curation note.
    pub note: Option<String>,
    /// Display derivatives, narrowest first.
    pub variants: Vec<Variant>,
    /// 1200x630 social card crop.
    pub og: Option<Variant>,
    /// When the Commons API was read (UTC, ISO 8601).
    pub retrieved: String,
}

impl Artwork {
    /// First verse of the passage, if inside the 66 books.
    #[must_use]
    pub fn start(&self) -> Option<Loc> {
        let p = self.passage.as_deref()?;
        xref::parse_one(p.split('-').next()?)
    }

    /// The widest display variant.
    #[must_use]
    pub fn largest(&self) -> Option<&Variant> {
        self.variants.iter().max_by_key(|v| v.width)
    }

    /// True when Commons files this work under the Public Domain Mark.
    #[must_use]
    pub fn pd_mark(&self) -> bool {
        self.licence_code == "pd" || self.categories.iter().any(|c| c == "CC-PD-Mark")
    }
}

/// The admitted art, in plan order.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// Every admitted work.
    pub works: Vec<Artwork>,
    /// Plan rows refused by the licence gate or the artist check, with the reason.
    pub refused: Vec<(String, String)>,
}

impl Manifest {
    /// Loads `art_manifest.json`.
    ///
    /// # Errors
    /// Fails when unreadable or malformed.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        serde_json::from_str(&raw).map_err(|e| Error::json("art_manifest.json", e))
    }

    /// Works whose passage starts in a given chapter, ordered by start verse.
    #[must_use]
    pub fn in_chapter(&self, book: usize, chapter: usize) -> Vec<&Artwork> {
        let mut v: Vec<&Artwork> = self
            .works
            .iter()
            .filter(|w| {
                w.start()
                    .is_some_and(|s| s.book == book && s.chapter == chapter)
            })
            .collect();
        v.sort_by_key(|w| w.start().map_or(0, |s| s.verse));
        v
    }

    /// Works whose passage starts in a book.
    #[must_use]
    pub fn in_book(&self, book: usize) -> Vec<&Artwork> {
        let mut v: Vec<&Artwork> = self
            .works
            .iter()
            .filter(|w| w.start().is_some_and(|s| s.book == book))
            .collect();
        v.sort_by_key(|w| w.start().map_or((0, 0), |s| (s.chapter, s.verse)));
        v
    }
}

/// The AG-CHILD exclusion list (`data/ag_child_removed.tsv`): Commons files that are
/// never used, whatever the plan or manifest says.
///
/// WHY a list in data and a gate in code: the content rule forbids any artwork showing
/// children unclothed. The audit decided per work; this list makes
/// that decision permanent, so a re-fetch or a hand-edited plan cannot bring a
/// removed work back.
#[derive(Debug, Default)]
pub struct Exclusions {
    files: std::collections::BTreeSet<String>,
}

impl Exclusions {
    /// Loads the list; a missing file means no exclusions.
    ///
    /// # Errors
    /// Fails only when the file exists and cannot be read.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        let files = raw
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split('\t').next())
            .map(|f| f.trim().to_string())
            .collect();
        Ok(Self { files })
    }

    /// True when a Commons file is excluded.
    #[must_use]
    pub fn excludes(&self, commons_file: &str) -> bool {
        self.files.contains(commons_file)
    }

    /// Number of excluded files.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True when nothing is excluded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl Manifest {
    /// Drops every excluded work and returns how many were dropped.
    pub fn apply(&mut self, ex: &Exclusions) -> usize {
        let before = self.works.len();
        self.works.retain(|w| !ex.excludes(&w.commons_file));
        before - self.works.len()
    }

    /// Keeps only works whose image was looked at and admitted.
    ///
    /// # Errors
    /// Refuses the whole build when any work has no recorded verdict: an image nobody
    /// looked at is never published, and a silent drop would hide the gap.
    pub fn apply_review(&mut self, review: &Review) -> Result<usize> {
        let unreviewed: Vec<&str> = self
            .works
            .iter()
            .filter(|w| review.verdict(&w.commons_file).is_none())
            .map(|w| w.commons_file.as_str())
            .collect();
        if !unreviewed.is_empty() {
            return Err(Error::Data(format!(
                "{} work(s) have no content-review verdict (first: {}); every image is looked at before it is published",
                unreviewed.len(),
                unreviewed.first().copied().unwrap_or_default()
            )));
        }
        let before = self.works.len();
        self.works
            .retain(|w| review.verdict(&w.commons_file) == Some(Verdict::Admit));
        Ok(before - self.works.len())
    }
}

/// A per-image verdict from the content review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Looked at and fit to publish.
    Admit,
    /// Looked at and refused (nudity, gore shown for shock, a duplicate, or unsure).
    Exclude,
}

/// One row of `data/content_review.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRow {
    /// Commons file title.
    pub commons_file: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Rule code: `OK`, `NUDITY`, `GORE`, `DUPLICATE`, `UNSURE` or `AG-CHILD`.
    pub rule: String,
    /// What was seen, in plain words.
    pub reason: String,
    /// Date the image was looked at (`YYYY-MM-DD`).
    pub reviewed: String,
    /// Who looked.
    pub reviewer: String,
}

/// The content review (`data/content_review.tsv`): one verdict per image, recorded by
/// the person or agent who looked at it.
///
/// WHY a gate in code: the content rule requires every image on
/// a public surface to be looked at one by one, with no nudity and no gore shown for
/// shock (clothed people of any age are fine; when unsure, leave it out). A title or
/// caption check is never enough, so the build refuses any work without a verdict.
#[derive(Debug, Default)]
pub struct Review {
    rows: std::collections::BTreeMap<String, ReviewRow>,
}

impl Review {
    /// Parses the review (tab-separated: file, verdict, rule, reason, reviewed, reviewer;
    /// `#` starts a comment).
    ///
    /// # Errors
    /// Fails on a malformed row, an unknown verdict, or a file reviewed twice.
    pub fn parse(raw: &str) -> Result<Self> {
        let mut rows = std::collections::BTreeMap::new();
        for (n, line) in raw.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').map(str::trim).collect();
            let [file, verdict, rule, reason, when, who] = f.as_slice() else {
                return Err(Error::Data(format!(
                    "content_review.tsv line {}: expected 6 tab-separated fields",
                    n + 1
                )));
            };
            let verdict = match *verdict {
                "ADMIT" => Verdict::Admit,
                "EXCLUDE" => Verdict::Exclude,
                other => {
                    return Err(Error::Data(format!(
                        "content_review.tsv line {}: unknown verdict {other:?}",
                        n + 1
                    )));
                }
            };
            let row = ReviewRow {
                commons_file: (*file).to_string(),
                verdict,
                rule: (*rule).to_string(),
                reason: (*reason).to_string(),
                reviewed: (*when).to_string(),
                reviewer: (*who).to_string(),
            };
            if rows.insert(row.commons_file.clone(), row).is_some() {
                return Err(Error::Data(format!(
                    "content_review.tsv line {}: {file} is reviewed twice",
                    n + 1
                )));
            }
        }
        Ok(Self { rows })
    }

    /// Loads the review; a missing file means no image has been reviewed.
    ///
    /// # Errors
    /// Fails when the file exists and cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        Self::parse(&raw)
    }

    /// The verdict for a Commons file, if it was reviewed.
    #[must_use]
    pub fn verdict(&self, commons_file: &str) -> Option<Verdict> {
        self.rows.get(commons_file).map(|r| r.verdict)
    }

    /// Every row, in file-title order.
    pub fn rows(&self) -> impl Iterator<Item = &ReviewRow> {
        self.rows.values()
    }
}

/// A URL slug from a scene title: lowercase ASCII words joined by `-`.
#[must_use]
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let mapped = match c {
            'à' | 'á' | 'â' | 'ä' => Some('a'),
            'è' | 'é' | 'ê' | 'ë' => Some('e'),
            'ì' | 'í' | 'î' | 'ï' => Some('i'),
            'ò' | 'ó' | 'ô' | 'ö' => Some('o'),
            'ù' | 'ú' | 'û' | 'ü' => Some('u'),
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        match mapped {
            Some(m) => out.push(m),
            None if !out.ends_with('-') && !out.is_empty() => out.push('-'),
            None => {}
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Strips HTML tags and collapses whitespace in a Commons metadata value.
#[must_use]
pub fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    let mut hidden = false;
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            let end = rest.find('>').unwrap_or(rest.len());
            let tag = rest.get(..end).unwrap_or("");
            if tag.contains("display: none") || tag.contains("display:none") {
                hidden = true;
                depth = 1;
            } else if hidden && tag.starts_with("<div") {
                depth += 1;
            } else if hidden && tag.starts_with("</div") {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    hidden = false;
                }
            }
            rest = rest.get(end + 1..).unwrap_or("");
            out.push(' ');
            continue;
        }
        if !hidden {
            out.push(c);
        }
        rest = rest.get(c.len_utf8()..).unwrap_or("");
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_keeps_words_and_folds_accents() {
        assert_eq!(slugify("The Creation of Light"), "the-creation-of-light");
        assert_eq!(slugify("Jacob's Dream"), "jacob-s-dream");
        assert_eq!(
            slugify("Doré \u{2014} The Judas Kiss"),
            "dore-the-judas-kiss"
        );
    }

    #[test]
    fn strip_markup_drops_hidden_wikidata_dates() {
        let raw = "1866<div style=\"display: none;\">date QS:P571,+1866-00-00T00:00:00Z/9</div>";
        assert_eq!(strip_markup(raw), "1866");
    }

    #[test]
    fn the_review_admits_only_what_was_looked_at_and_refuses_unreviewed_work() {
        let review = Review::parse(
            "# file\tverdict\trule\treason\treviewed\treviewer\nFile:a.jpg\tADMIT\tOK\tclothed figures\t2026-10-19\tagent\nFile:b.jpg\tEXCLUDE\tNUDITY\tunclothed figure\t2026-10-19\tagent\n",
        );
        assert!(review.is_ok());
        let review = review.unwrap_or_default();
        assert_eq!(review.verdict("File:a.jpg"), Some(Verdict::Admit));
        assert_eq!(review.verdict("File:b.jpg"), Some(Verdict::Exclude));
        assert_eq!(review.verdict("File:c.jpg"), None);
        assert!(Review::parse("File:a.jpg\tMAYBE\tOK\tx\t2026-10-19\tme\n").is_err());
        assert!(
            Review::parse(
                "File:a.jpg\tADMIT\tOK\tx\t2026-10-19\tme\nFile:a.jpg\tADMIT\tOK\tx\t2026-10-19\tme\n"
            )
            .is_err()
        );
    }

    #[test]
    fn plan_rows_need_six_fields() {
        let ok = parse_plan("File:a.jpg\tdore\tA\tGen.1.1\t-\t-\n# c\n");
        assert_eq!(ok.map(|r| r.len()).ok(), Some(1));
        assert!(parse_plan("File:a.jpg\tdore\n").is_err());
    }
}
