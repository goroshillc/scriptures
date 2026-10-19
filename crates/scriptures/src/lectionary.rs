//! The Orthodox lectionary: a dated window of days from orthocal.info, read at build
//! time and served as a static page.
//!
//! > *"This is the day which the LORD hath made; we will rejoice and be glad in it."*
//! > - Psalms 118:24, KJV
//!
//! WHY a snapshot: the site has no server, so "today's readings" are fetched when the
//! site is built and published as a window of dates (each with an anchor). The daily
//! rebuild described in DEPLOY.md keeps it current; the optional script marks the
//! visitor's own date. Only facts are kept - day titles, saints commemorated, fast,
//! tone and reading references - never orthocal's narrative texts, whose rights are
//! not ours to republish.

use crate::canon;
use crate::commons::civil;
use crate::net::Net;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// One reading of the day, resolved to the reader's own pages when possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    /// "Epistle", "Gospel", "Vespers", ...
    pub source: String,
    /// Display reference as orthocal gives it ("Luke 7.36-50").
    pub display: String,
    /// Canon index of the first verse's book, when inside the 66.
    pub book: Option<usize>,
    /// First chapter and verse.
    pub start: Option<(usize, usize)>,
}

/// One calendar day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Day {
    /// ISO date.
    pub date: String,
    /// Week-day title(s) ("Monday of the 20th week after Pentecost").
    pub titles: Vec<String>,
    /// Feast-level description.
    pub feast: String,
    /// Fast description.
    pub fast: String,
    /// Tone of the week.
    pub tone: Option<u64>,
    /// Saints and feasts commemorated.
    pub saints: Vec<String>,
    /// Readings.
    pub readings: Vec<Reading>,
}

/// The fetched window.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Lectionary {
    /// When fetched (UTC date).
    pub fetched: String,
    /// The calendar used.
    pub calendar: String,
    /// Days in date order.
    pub days: Vec<Day>,
}

impl Lectionary {
    /// Loads `data/lectionary.json`.
    ///
    /// # Errors
    /// Fails when unreadable or malformed.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
        serde_json::from_str(&raw).map_err(|e| Error::json("lectionary.json", e))
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    x.as_str()
                        .map(str::to_string)
                        .or_else(|| x.get("title").and_then(Value::as_str).map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Parses one orthocal day document.
#[must_use]
pub fn parse_day(date: &str, v: &Value) -> Day {
    let readings = v
        .get("readings")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .map(|r| {
                    let first = r.pointer("/passage/0");
                    let book = first
                        .and_then(|p| p.get("book"))
                        .and_then(Value::as_str)
                        .and_then(canon::index_of_usfm);
                    let ch = first
                        .and_then(|p| p.get("chapter"))
                        .and_then(Value::as_u64)
                        .and_then(|n| usize::try_from(n).ok());
                    let vs = first
                        .and_then(|p| p.get("verse"))
                        .and_then(Value::as_u64)
                        .and_then(|n| usize::try_from(n).ok());
                    Reading {
                        source: text(r, "source"),
                        display: text(r, "display"),
                        book,
                        start: ch.zip(vs),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let fast = [text(v, "fast_level_desc"), text(v, "fast_exception_desc")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    Day {
        date: date.to_string(),
        titles: strings(v.get("titles")),
        feast: text(v, "feast_level_description"),
        fast,
        tone: v.get("tone").and_then(Value::as_u64).filter(|t| *t > 0),
        saints: strings(v.get("saints")),
        readings,
    }
}

/// Fetches `days` days starting `before` days before today (UTC).
///
/// # Errors
/// Fails when orthocal cannot be reached or answers with something other than JSON.
pub fn fetch(net: &mut Net, before: i64, days: i64) -> Result<Lectionary> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let today = i64::try_from(secs / 86_400).unwrap_or(0);
    let mut out = Lectionary {
        fetched: {
            let (y, m, d) = civil(today);
            format!("{y:04}-{m:02}-{d:02}")
        },
        calendar: "Gregorian (New Calendar), orthocal.info default jurisdiction".to_string(),
        days: Vec::new(),
    };
    for offset in -before..(days - before) {
        let (y, m, d) = civil(today + offset);
        let url = format!("https://orthocal.info/api/gregorian/{y}/{m}/{d}/");
        let v = net.get_json(&url)?;
        let date = format!("{y:04}-{m:02}-{d:02}");
        out.days.push(parse_day(&date, &v));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gospel_reading_resolves_to_luke_seven_thirty_six() {
        let v = serde_json::json!({
            "titles": ["Monday of the 20th week after Pentecost"],
            "saints": ["St Symeon the New Theologian (1022)"],
            "tone": 2, "fast_level_desc": "No Fast", "fast_exception_desc": "",
            "readings": [{"source": "Gospel", "display": "Luke 7.36-50",
                "passage": [{"book": "LUK", "chapter": 7, "verse": 36}]}]
        });
        let day = parse_day("2026-10-12", &v);
        let r = day.readings.first().cloned();
        assert_eq!(r.as_ref().and_then(|r| r.book), Some(41));
        assert_eq!(r.and_then(|r| r.start), Some((7, 36)));
        assert_eq!(day.fast, "No Fast");
        assert_eq!(day.tone, Some(2));
    }
}
