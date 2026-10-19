//! Fetches the curated art from Wikimedia Commons, gated on the licence the Commons
//! API reports for each file.
//!
//! > *"Heal the sick, cleanse the lepers, raise the dead, cast out devils: freely ye
//! > have received, freely give."* - Matthew 10:8, KJV
//!
//! WHY the gate lives here: a file's licence is read from the source's own API
//! (`extmetadata`), never from a badge or memory. Only `pd` and `cc0` pass; anything
//! else (CC BY-SA photographs of paintings, unlicensed scans) is refused and listed in
//! the manifest with the licence string that refused it. The artist named in the plan
//! must also match what Commons states, so a mislabelled file cannot slip in.

use crate::art::{self, Artwork, Manifest, PlanRow};
use crate::imaging;
use crate::net::{self, Net};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

/// Master width requested from Commons: the widest display derivative.
///
/// 1920 is a 2x rendition of the 60rem plate column and one of Wikimedia's standard
/// thumbnail steps, so the request is served from its cache, not rendered on demand.
pub const MASTER_WIDTH: u32 = 1920;

/// Canonical credit facts per artist key.
#[derive(Debug, Clone, Copy)]
pub struct ArtistFacts {
    /// Name as credited.
    pub name: &'static str,
    /// Life dates.
    pub dates: &'static str,
    /// Medium.
    pub medium: &'static str,
    /// Lower-case fragments that must appear in what Commons says about the file.
    pub must_match: &'static [&'static str],
    /// Date used when Commons gives no year from the artist's lifetime.
    pub fallback_date: &'static str,
}

/// Looks up the credit facts for an artist key from the plan.
#[must_use]
pub fn artist_facts(key: &str) -> Option<ArtistFacts> {
    const DORE: &[&str] = &["doré", "dore"];
    const DURER: &[&str] = &["dürer", "durer"];
    const TITIAN: &[&str] = &["titian", "tiziano", "titien", "vecellio", "tizian"];
    let f = |name, dates, medium, must_match, fallback_date| ArtistFacts {
        name,
        dates,
        medium,
        must_match,
        fallback_date,
    };
    match key {
        "dore" => Some(f(
            "Gustave Doré",
            "1832–1883",
            "Wood engraving",
            DORE,
            "1866",
        )),
        "dore-painting" => Some(f("Gustave Doré", "1832–1883", "Oil on canvas", DORE, "")),
        "durer" => Some(f("Albrecht Dürer", "1471–1528", "Woodcut", DURER, "")),
        "durer-engraving" => Some(f("Albrecht Dürer", "1471–1528", "Engraving", DURER, "")),
        "titian" => Some(f(
            "Titian (Tiziano Vecellio)",
            "c. 1488–1576",
            "Oil on canvas",
            TITIAN,
            "",
        )),
        _ => None,
    }
}

/// The first year from the artist's working centuries found in a Commons date string.
#[must_use]
pub fn period_year(date: &str) -> Option<String> {
    let bytes: Vec<char> = date.chars().collect();
    bytes.windows(4).find_map(|w| {
        let s: String = w.iter().collect();
        let n: u32 = s.parse().ok()?;
        (1400..1900).contains(&n).then_some(s)
    })
}

fn ext_value(em: &Value, key: &str) -> String {
    em.get(key)
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .map(art::strip_markup)
        .unwrap_or_default()
}

fn query_batch(net: &mut Net, titles: &[&str]) -> Result<HashMap<String, Value>> {
    let joined = titles.join("|");
    let url = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=imageinfo&iiprop=extmetadata%7Csize%7Csha1%7Curl%7Cmime&iiurlwidth={MASTER_WIDTH}&titles={}",
        net::encode(&joined)
    );
    let v = net.get_json(&url)?;
    let mut alias: HashMap<String, String> = HashMap::new();
    for n in v
        .pointer("/query/normalized")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(from), Some(to)) = (
            n.get("from").and_then(Value::as_str),
            n.get("to").and_then(Value::as_str),
        ) {
            alias.insert(to.to_string(), from.to_string());
        }
    }
    let mut out = HashMap::new();
    if let Some(pages) = v.pointer("/query/pages").and_then(Value::as_object) {
        for page in pages.values() {
            let Some(title) = page.get("title").and_then(Value::as_str) else {
                continue;
            };
            let key = alias
                .get(title)
                .cloned()
                .unwrap_or_else(|| title.to_string());
            out.insert(key, page.clone());
        }
    }
    Ok(out)
}

/// Whether a Commons licence passes the gate: public domain or CC0 only.
#[must_use]
pub fn licence_passes(code: &str, short: &str) -> bool {
    let c = code.to_ascii_lowercase();
    let s = short.to_ascii_lowercase();
    c == "pd" || c == "cc0" || s == "public domain" || s == "cc0"
}

fn now_iso() -> String {
    // Days since the epoch to a civil date (Howard Hinnant's algorithm), so the
    // retrieval stamp needs no date crate.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let tod = secs % 86_400;
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Civil date from days since 1970-01-01.
#[must_use]
pub const fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Today's UTC date as `YYYY-MM-DD`.
#[must_use]
pub fn today_utc() -> String {
    now_iso().chars().take(10).collect()
}

/// Fetches every plan row, gates it, downloads a master, derives the display images,
/// and returns the manifest.
///
/// # Errors
/// Fails on unreadable plan, network failure, or an image that cannot be processed.
/// A refused licence is not an error: it is recorded in `Manifest::refused`.
pub fn fetch(plan_path: &Path, cache: &Path, net: &mut Net) -> Result<Manifest> {
    let raw = std::fs::read_to_string(plan_path).map_err(|e| Error::io("read", plan_path, e))?;
    // Every plan row is fetched, including works on the permanent exclusion list, so the
    // content review can look at each one and record its verdict. Publishing is refused
    // at build time (`site::Inputs::load`), which is where the guarantee lives.
    let plan: Vec<PlanRow> = art::parse_plan(&raw)?;
    let masters = cache.join("masters");
    let derived = cache.join("derived");
    std::fs::create_dir_all(&masters).map_err(|e| Error::io("create dir", &masters, e))?;
    let mut pages: HashMap<String, Value> = HashMap::new();
    let titles: Vec<&str> = plan.iter().map(|r| r.file.as_str()).collect();
    for chunk in titles.chunks(40) {
        pages.extend(query_batch(net, chunk)?);
    }
    let mut manifest = Manifest::default();
    let mut seen_slugs: HashMap<String, usize> = HashMap::new();
    for row in &plan {
        match admit(row, &pages, &masters, &derived, net, &mut seen_slugs) {
            Ok(work) => {
                eprintln!("  admitted {} ({})", work.scene, work.licence_short);
                manifest.works.push(work);
            }
            Err(Error::Licence { file, reason }) => {
                eprintln!("  REFUSED {file}: {reason}");
                manifest.refused.push((file, reason));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(manifest)
}

fn u32_field(info: &Value, key: &str) -> u32 {
    info.get(key)
        .and_then(Value::as_u64)
        .and_then(|w| u32::try_from(w).ok())
        .unwrap_or(0)
}

fn str_field(info: &Value, key: &str) -> String {
    info.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Downloads a master once; a later run reuses the cached file. Returns the path and
/// the record of exactly which bytes it holds.
fn fetch_master(
    net: &mut Net,
    masters: &Path,
    slug: &str,
    src: &str,
) -> Result<(std::path::PathBuf, art::MasterRecord)> {
    let is_png = Path::new(src.split('?').next().unwrap_or(src))
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    let master = masters.join(format!("{slug}.{}", if is_png { "png" } else { "jpg" }));
    if !std::fs::metadata(&master).is_ok_and(|m| m.len() > 0) {
        let bytes = net.get(src)?;
        std::fs::write(&master, bytes).map_err(|e| Error::io("write", &master, e))?;
    }
    let bytes = std::fs::read(&master).map_err(|e| Error::io("read", &master, e))?;
    let (width, height) = image::image_dimensions(&master).map_err(|e| Error::Image {
        path: master.clone(),
        reason: e.to_string(),
    })?;
    let record = art::MasterRecord {
        url: src.to_string(),
        width,
        height,
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        sha256: crate::site::sha256_hex(&bytes),
    };
    Ok((master, record))
}

fn admit(
    row: &PlanRow,
    pages: &HashMap<String, Value>,
    masters: &Path,
    derived: &Path,
    net: &mut Net,
    seen_slugs: &mut HashMap<String, usize>,
) -> Result<Artwork> {
    let refuse = |reason: String| Error::Licence {
        file: row.file.clone(),
        reason,
    };
    let facts = artist_facts(&row.artist)
        .ok_or_else(|| refuse(format!("unknown artist key {}", row.artist)))?;
    let page = pages
        .get(&row.file)
        .ok_or_else(|| refuse("not returned by the Commons API".to_string()))?;
    if page.get("missing").is_some() {
        return Err(refuse("file does not exist on Commons".to_string()));
    }
    let info = page
        .pointer("/imageinfo/0")
        .ok_or_else(|| refuse("no imageinfo".to_string()))?;
    let em = info.get("extmetadata").cloned().unwrap_or(Value::Null);
    let licence_code = ext_value(&em, "License");
    let licence_short = ext_value(&em, "LicenseShortName");
    if !licence_passes(&licence_code, &licence_short) {
        return Err(refuse(format!(
            "licence {licence_short:?} (code {licence_code:?})"
        )));
    }
    let artist = ext_value(&em, "Artist");
    let description = ext_value(&em, "ImageDescription");
    let haystack = format!("{artist} {description} {}", row.file).to_lowercase();
    if !facts.must_match.iter().any(|m| haystack.contains(m)) {
        return Err(refuse(format!(
            "artist check failed: Commons artist {artist:?}"
        )));
    }
    let date_raw = ext_value(&em, "DateTimeOriginal");
    let date = period_year(&date_raw).unwrap_or_else(|| facts.fallback_date.to_string());
    let categories: Vec<String> = ext_value(&em, "Categories")
        .split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let width = u32_field(info, "width");
    let height = u32_field(info, "height");
    let src_key = if width > MASTER_WIDTH {
        "thumburl"
    } else {
        "url"
    };
    let src = str_field(info, src_key);
    if src.is_empty() {
        return Err(refuse("no download URL".to_string()));
    }
    let base = art::slugify(&row.scene);
    let n = seen_slugs.entry(base.clone()).or_insert(0);
    *n += 1;
    let slug = if *n == 1 { base } else { format!("{base}-{n}") };
    let (master, master_record) = fetch_master(net, masters, &slug, &src)?;
    let (variants, og) = imaging::derive(&master, derived, &slug)?;
    Ok(Artwork {
        slug,
        commons_file: row.file.clone(),
        commons_url: str_field(info, "descriptionurl"),
        artist_key: row.artist.clone(),
        artist: if artist.is_empty() {
            facts.name.to_string()
        } else {
            artist
        },
        artist_dates: facts.dates.to_string(),
        date,
        scene: row.scene.clone(),
        medium: facts.medium.to_string(),
        commons_description: description,
        licence_short,
        licence_code,
        usage_terms: ext_value(&em, "UsageTerms"),
        licence_url: Some(ext_value(&em, "LicenseUrl")).filter(|s| !s.is_empty()),
        categories,
        sha1: str_field(info, "sha1"),
        original_size: (width, height),
        original_bytes: info.get("size").and_then(Value::as_u64).unwrap_or(0),
        master: Some(master_record),
        passage: row.passage.clone(),
        other_ref: row.other_ref.clone(),
        note: row.note.clone(),
        variants,
        og,
        retrieved: now_iso(),
    })
}

/// Licence evidence for the site icon, written beside it as `favicon.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IconRecord {
    /// Commons file title.
    pub commons_file: String,
    /// Commons description page.
    pub commons_url: String,
    /// Uploader or author as Commons states it.
    pub artist: String,
    /// Commons `LicenseShortName`, verbatim.
    pub licence_short: String,
    /// Commons `License` code, verbatim.
    pub licence_code: String,
    /// When retrieved.
    pub retrieved: String,
}

/// Inserts a `viewBox` (so the mark scales instead of clipping) and an RDF
/// `<metadata>` block after the opening `<svg …>` tag. No path is touched.
#[must_use]
pub fn stamp_svg(svg: &str, rights: &str) -> String {
    let Some(open) = svg.find("<svg") else {
        return svg.to_string();
    };
    let Some(close_rel) = svg.get(open..).and_then(|t| t.find('>')) else {
        return svg.to_string();
    };
    let close = open + close_rel;
    let mut tag = svg.get(open..close).unwrap_or("").to_string();
    if !tag.contains("viewBox") {
        let attr = |name: &str| -> Option<String> {
            let key = format!(" {name}=\"");
            let start = tag.find(&key)? + key.len();
            let end = tag.get(start..)?.find('"')? + start;
            tag.get(start..end).map(str::to_string)
        };
        if let (Some(w), Some(h)) = (attr("width"), attr("height")) {
            let _ = write!(tag, " viewBox=\"0 0 {w} {h}\"");
        }
    }
    let meta = format!(
        "<metadata><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><rdf:Description rdf:about=\"\"><dc:rights>{}</dc:rights><dc:publisher>The Scriptures, Goroshi LLC</dc:publisher></rdf:Description></rdf:RDF></metadata>",
        crate::meta::xml_escape(rights)
    );
    format!(
        "{}{tag}>{meta}{}",
        svg.get(..open).unwrap_or(""),
        svg.get(close + 1..).unwrap_or("")
    )
}

/// Fetches a public-domain SVG from Commons for the site icon, gated like the art.
///
/// # Errors
/// Fails when the licence is not public domain or CC0, or on network errors.
pub fn fetch_icon(net: &mut Net, title: &str, data: &Path) -> Result<IconRecord> {
    fetch_svg(
        net,
        title,
        &data.join("favicon.svg"),
        &data.join("favicon.json"),
        "the site icon",
    )
}

/// Fetches one public-domain or CC0 SVG from Commons, stamps its metadata, and writes
/// the licence evidence beside it.
///
/// # Errors
/// Fails when the licence is not public domain or CC0, or on network errors.
pub fn fetch_svg(
    net: &mut Net,
    title: &str,
    out_svg: &Path,
    out_json: &Path,
    used_as: &str,
) -> Result<IconRecord> {
    let pages = query_batch(net, &[title])?;
    let page = pages
        .get(title)
        .ok_or_else(|| Error::Data(format!("{title} not returned")))?;
    let info = page
        .pointer("/imageinfo/0")
        .ok_or_else(|| Error::Data(format!("{title}: no imageinfo")))?;
    let em = info.get("extmetadata").cloned().unwrap_or(Value::Null);
    let licence_code = ext_value(&em, "License");
    let licence_short = ext_value(&em, "LicenseShortName");
    if !licence_passes(&licence_code, &licence_short) {
        return Err(Error::Licence {
            file: title.to_string(),
            reason: licence_short,
        });
    }
    let url = info
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Data("no url".into()))?;
    let svg = String::from_utf8(net.get(url)?)
        .map_err(|e| Error::Data(format!("SVG is not UTF-8: {e}")))?;
    let record = IconRecord {
        commons_file: title.to_string(),
        commons_url: str_field(info, "descriptionurl"),
        artist: ext_value(&em, "Artist"),
        licence_short,
        licence_code,
        retrieved: now_iso(),
    };
    let rights = format!(
        "{} ({}), {} per Wikimedia Commons; no copyright is claimed in it. Used as {used_as} of The Scriptures, Goroshi LLC.",
        record.commons_file, record.commons_url, record.licence_short
    );
    if let Some(parent) = out_svg.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io("create dir", parent, e))?;
    }
    std::fs::write(out_svg, stamp_svg(&svg, &rights))
        .map_err(|e| Error::io("write", out_svg, e))?;
    let json = serde_json::to_string_pretty(&record).map_err(|e| Error::json("svg record", e))?;
    std::fs::write(out_json, json).map_err(|e| Error::io("write", out_json, e))?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_svg_adds_a_viewbox_and_metadata_without_touching_paths() {
        let svg = "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"12.5\" height=\"20\"><path d=\"M0 0h1\"/></svg>";
        let out = stamp_svg(svg, "PD");
        assert!(out.contains("viewBox=\"0 0 12.5 20\">"));
        assert!(out.contains("<dc:rights>PD</dc:rights>"));
        assert!(out.contains("<path d=\"M0 0h1\"/>"));
    }

    #[test]
    fn only_public_domain_and_cc0_pass_the_gate() {
        assert!(licence_passes("pd", "Public domain"));
        assert!(licence_passes("cc0", "CC0"));
        assert!(!licence_passes("cc-by-sa-3.0", "CC BY-SA 3.0"));
        assert!(!licence_passes("", ""));
    }

    #[test]
    fn period_year_ignores_upload_timestamps() {
        assert_eq!(period_year("1866-01-01").as_deref(), Some("1866"));
        assert_eq!(period_year("2012-12-01 13:14:47"), None);
        assert_eq!(
            period_year("between 1866 and 1870"),
            Some("1866".to_string())
        );
    }

    #[test]
    fn the_epoch_and_a_leap_day_convert_to_civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_782), (2024, 2, 29));
    }
}
