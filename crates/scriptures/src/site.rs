//! Site configuration and the build: inputs in, a complete static site out.
//!
//! > *"For which of you, intending to build a tower, sitteth not down first, and
//! > counteth the cost, whether he have sufficient to finish it?"* - Luke 14:28, KJV
//!
//! WHY a marker file: the build replaces its output directory wholesale so no stale
//! page survives a rename, and it only ever removes a directory that carries the
//! marker it wrote itself - never a directory it did not create.

use crate::art::Manifest;
use crate::bible::Bible;
use crate::canon;
use crate::commons::artist_facts;
use crate::lectionary::Lectionary;
use crate::meta::{self, ImageMeta};
use crate::redletter::RedLetters;
use crate::render::{self, Ctx, Served};
use crate::seo::{self, UrlEntry};
use crate::xref::{self, Xrefs};
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Marker written into every output directory this tool creates.
pub const MARKER: &str = ".goroshi-scriptures-dist";

/// Where the site lives and when it was built.
#[derive(Debug, Clone)]
pub struct SiteConfig {
    /// Scheme and host (`https://scriptures.goroshillc.com`).
    pub origin: String,
    /// Path prefix ending in `/` (`/` or `/scriptures/`).
    pub base_path: String,
    /// Build date, `YYYY-MM-DD` (UTC).
    pub build_date: String,
    /// Whether a favicon file is shipped.
    pub has_icon: bool,
    /// Site-relative path of the regular reading face (EB Garamond), when shipped.
    pub font_regular: Option<String>,
    /// Site-relative path of the italic reading face, when shipped.
    pub font_italic: Option<String>,
    /// Optional prebuilt Rust discovery WebAssembly and generated browser binding.
    pub search_assets: Option<PathBuf>,
    /// Build-bound cache key for the discovery catalogue, glue and WASM.
    pub discovery_version: String,
    /// Which edition to write: the website, or the offline desktop app.
    pub edition: Edition,
}

/// The two editions one build can write.
///
/// WHY a desktop edition: the website serves every plate in AVIF, WebP and JPEG at six
/// widths plus the full scan (about 5 GB), which a browser downloads one image at a time.
/// An installer carries every file at once, so the desktop app ships JPEG only (every
/// webview reads it, and it keeps the embedded credit metadata) at three widths, with the
/// zoom view capped at [`DESKTOP_FULL_MAX`] pixels: about 0.5 GB. Text, links, search and
/// the private record are identical in both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Edition {
    /// Every format and width, for the open web.
    #[default]
    Web,
    /// JPEG at [`DESKTOP_WIDTHS`] plus one zoom image, for the offline app.
    Desktop,
}

impl Edition {
    /// Parses `web` or `desktop`.
    ///
    /// # Errors
    /// Fails on any other name.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "web" => Ok(Self::Web),
            "desktop" => Ok(Self::Desktop),
            other => Err(Error::Usage(format!(
                "unknown edition {other} (web or desktop)"
            ))),
        }
    }
}

/// Display widths the desktop edition keeps below its zoom image.
pub const DESKTOP_WIDTHS: [u32; 2] = [480, 960];
/// The widest image the desktop edition ships (its zoom view).
pub const DESKTOP_FULL_MAX: u32 = 1_920;

/// The crisp variants the desktop edition ships, each marked when it is the zoom image.
///
/// They are the [`DESKTOP_WIDTHS`] narrower than the zoom image, then the widest variant
/// no wider than [`DESKTOP_FULL_MAX`] as the zoom image.
#[must_use]
pub fn desktop_variants(
    variants: &[crate::crisp::CrispVariant],
) -> Vec<(&crate::crisp::CrispVariant, bool)> {
    let Some(zoom) = variants
        .iter()
        .filter(|v| v.width <= DESKTOP_FULL_MAX)
        .max_by_key(|v| v.width)
    else {
        return Vec::new();
    };
    let mut out: Vec<(&crate::crisp::CrispVariant, bool)> = variants
        .iter()
        .filter(|v| v.width < zoom.width && DESKTOP_WIDTHS.contains(&v.width))
        .map(|v| (v, false))
        .collect();
    out.push((zoom, true));
    out
}

impl SiteConfig {
    /// Parses a base URL such as `https://goroshillc.com/scriptures`.
    ///
    /// # Errors
    /// Fails when the URL is not `http(s)://host[/path]`.
    pub fn new(base_url: &str, build_date: &str) -> Result<Self> {
        let (scheme, rest) = base_url
            .split_once("://")
            .ok_or_else(|| Error::Usage(format!("base URL needs a scheme: {base_url}")))?;
        if scheme != "https" && scheme != "http" {
            return Err(Error::Usage(format!(
                "base URL scheme must be http or https: {base_url}"
            )));
        }
        let (host, path) = rest.split_once('/').map_or((rest, ""), |(h, p)| (h, p));
        if host.is_empty() {
            return Err(Error::Usage(format!("base URL needs a host: {base_url}")));
        }
        let trimmed = path.trim_matches('/');
        let base_path = if trimmed.is_empty() {
            "/".to_string()
        } else {
            format!("/{trimmed}/")
        };
        Ok(Self {
            origin: format!("{scheme}://{host}"),
            base_path,
            build_date: build_date.to_string(),
            has_icon: false,
            font_regular: None,
            font_italic: None,
            search_assets: None,
            discovery_version: String::new(),
            edition: Edition::Web,
        })
    }

    /// Root-relative link to a site path (`"john/3/"` -> `"/john/3/"`).
    #[must_use]
    pub fn href(&self, rel: &str) -> String {
        format!("{}{}", self.base_path, rel.trim_start_matches('/'))
    }

    /// Absolute URL of a site path.
    #[must_use]
    pub fn abs(&self, rel: &str) -> String {
        format!("{}{}", self.origin, self.href(rel))
    }

    /// The copyright year (the build year).
    #[must_use]
    pub fn year(&self) -> &str {
        self.build_date.get(..4).unwrap_or("2026")
    }

    /// The icon link (an empty data URL when no icon ships, which stops the browser
    /// requesting `/favicon.ico` and logging a 404).
    #[must_use]
    pub fn icon_href(&self) -> String {
        if self.has_icon {
            self.href(&icon_file())
        } else {
            "data:,".to_string()
        }
    }
}

/// The icon's content-addressed file name.
///
/// Pages link this name, so neither the edge cache nor a browser can keep showing an older
/// icon (on 2026-10-19 a CDN kept serving the previous `/favicon.svg` for its `max-age`
/// after the emblem changed).
#[must_use]
pub fn icon_file() -> String {
    let hash = sha256_hex(chi_rho::favicon().as_bytes());
    format!("favicon-{}.svg", hash.get(..16).unwrap_or(&hash))
}

/// Writes the site icon: the reader's own emblem, the Chi Rho in its halo, from the shared
/// `chi-rho` crate (black on white, white on black when the visitor's system is dark). Pages
/// link the content-addressed copy; `favicon.svg` stays for clients that ask for it by name.
fn write_icon(out: &Path) -> Result<()> {
    let icon = chi_rho::favicon();
    write(out, "favicon.svg", icon.as_bytes())?;
    write(out, &icon_file(), icon.as_bytes())?;
    Ok(())
}

/// Counts reported after a build.
#[derive(Debug, Default)]
pub struct Report {
    /// HTML pages written.
    pub pages: usize,
    /// Chapter pages.
    pub chapters: usize,
    /// Book pages.
    pub books: usize,
    /// Art pages.
    pub art_pages: usize,
    /// Image files written (all carry XMP and IPTC).
    pub images: usize,
    /// Total bytes of HTML.
    pub html_bytes: u64,
    /// Total bytes of images.
    pub image_bytes: u64,
    /// Largest page (path, bytes).
    pub largest_page: (String, u64),
    /// Sitemap URLs.
    pub sitemap_urls: usize,
}

/// sha256 hex of bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn write(out: &Path, rel: &str, bytes: &[u8]) -> Result<u64> {
    let path = out.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io("create dir", parent, e))?;
    }
    std::fs::write(&path, bytes).map_err(|e| Error::io("write", &path, e))?;
    u64::try_from(bytes.len()).map_err(|_| Error::Data("file too large".into()))
}

fn prepare_out(out: &Path) -> Result<()> {
    if out.exists() {
        return Err(Error::Data(format!(
            "{} already exists; preserve it and choose a new versioned output",
            out.display()
        )));
    }
    std::fs::create_dir_all(out).map_err(|e| Error::io("create dir", out, e))?;
    std::fs::write(out.join(MARKER), b"built by goroshi-scriptures\n")
        .map_err(|e| Error::io("write", out.join(MARKER), e))
}

/// The inputs the build reads, all from `data/`.
pub struct Inputs {
    /// KJV.
    pub bible: Bible,
    /// Cross-references.
    pub xrefs: Xrefs,
    /// Red letters.
    pub red: RedLetters,
    /// Art manifest.
    pub art: Manifest,
    /// Lectionary window (optional).
    pub lect: Option<Lectionary>,
    /// Directory of derived images.
    pub derived: PathBuf,
    /// Favicon SVG bytes, when shipped.
    pub icon: Option<Vec<u8>>,
    /// Licence evidence for the favicon.
    pub icon_record: Option<crate::commons::IconRecord>,
    /// The reading face: (file name, bytes) for the regular and italic subsets, from
    /// `data/fonts/`, with the font's licence text.
    pub fonts: Vec<(String, Vec<u8>)>,
    /// Psalm 115:1 with the site's motto and the verified original languages.
    pub psalm: Option<crate::features::Psalm115>,
    /// The computus compiled to WebAssembly (`data/computus.wasm`).
    pub wasm: Option<Vec<u8>>,
    /// The crisp derivatives (AVIF, WebP, JPEG up to the full scan), when derived.
    pub crisp: Option<crate::crisp::CrispManifest>,
    /// Directory of the crisp derivatives.
    pub crisp_dir: PathBuf,
}

/// Reads the shipped font files (`data/fonts/*.woff` and `OFL.txt`), if any.
fn load_fonts(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for name in [
        "EBGaramond-Text.woff",
        "EBGaramond-Italic-Text.woff",
        "OFL.txt",
    ] {
        let path = dir.join(name);
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
            out.push((name.to_string(), bytes));
        }
    }
    Ok(out)
}

impl Inputs {
    /// Loads everything from a data directory.
    ///
    /// # Errors
    /// Fails when a required input is missing or inconsistent.
    pub fn load(data: &Path) -> Result<Self> {
        let kjv_path = data.join("kjv.json");
        let kjv_bytes = std::fs::read(&kjv_path).map_err(|e| Error::io("read", &kjv_path, e))?;
        let bible = Bible::from_json(&String::from_utf8_lossy(&kjv_bytes))?;
        let xrefs = Xrefs::load(&data.join("xrefs.json"))?;
        let red = RedLetters::load(&data.join("redletter.json"), &sha256_hex(&kjv_bytes))?;
        let mut art = Manifest::load(&data.join("art_manifest.json"))?;
        let ex = crate::art::Exclusions::load(&data.join("ag_child_removed.tsv"))?;
        let dropped = art.apply(&ex);
        if dropped > 0 {
            eprintln!("AG-CHILD gate: {dropped} excluded work(s) dropped from the manifest");
        }
        let review = crate::art::Review::load(&data.join("content_review.tsv"))?;
        let refused = art.apply_review(&review)?;
        eprintln!(
            "content review: {} work(s) admitted, {refused} excluded by verdict",
            art.works.len()
        );
        let lect_path = data.join("lectionary.json");
        let lect = if lect_path.exists() {
            Some(Lectionary::load(&lect_path)?)
        } else {
            None
        };
        let icon_path = data.join("favicon.svg");
        let icon = if icon_path.exists() {
            Some(std::fs::read(&icon_path).map_err(|e| Error::io("read", &icon_path, e))?)
        } else {
            None
        };
        let rec_path = data.join("favicon.json");
        let icon_record = if icon.is_some() && rec_path.exists() {
            let raw =
                std::fs::read_to_string(&rec_path).map_err(|e| Error::io("read", &rec_path, e))?;
            Some(serde_json::from_str(&raw).map_err(|e| Error::json("favicon.json", e))?)
        } else {
            None
        };
        Ok(Self {
            bible,
            xrefs,
            red,
            art,
            lect,
            derived: data.join("art-cache").join("derived"),
            icon,
            icon_record,
            fonts: load_fonts(&data.join("fonts"))?,
            psalm: {
                let p = data.join("psalm115.json");
                if p.exists() {
                    let raw = std::fs::read_to_string(&p).map_err(|e| Error::io("read", &p, e))?;
                    Some(serde_json::from_str(&raw).map_err(|e| Error::json("psalm115.json", e))?)
                } else {
                    None
                }
            },
            wasm: {
                let w = data.join("computus.wasm");
                if w.exists() {
                    Some(std::fs::read(&w).map_err(|e| Error::io("read", &w, e))?)
                } else {
                    None
                }
            },
            crisp: crate::crisp::CrispManifest::load(data)?,
            crisp_dir: crate::crisp::crisp_dir(data),
        })
    }
}

fn image_meta(inp: &Inputs, cfg: &SiteConfig, w: &crate::art::Artwork) -> ImageMeta {
    let facts = artist_facts(&w.artist_key);
    let artist = facts.map_or_else(|| w.artist.clone(), |f| f.name.to_string());
    let dates = facts.map_or("", |f| f.dates);
    let luke = inp.bible.verse_text(41, 1, 37).unwrap_or_default();
    let illustrates = match (w.start(), &w.other_ref) {
        (Some(s), _) => {
            let label = render::pretty_range(&inp.bible, w.passage.as_deref().unwrap_or(""));
            let first = inp
                .bible
                .verse_text(s.book, s.chapter, s.verse)
                .unwrap_or_default();
            let first_ref = xref::human(&inp.bible, &xref::osis_key(s.book, s.chapter, s.verse));
            format!("Illustrates {label}. \u{201c}{first}\u{201d} ({first_ref}, KJV).")
        }
        (None, Some(r)) => format!(
            "Illustrates {r}, a book read in the Orthodox Church beyond the 66 books of the King James text."
        ),
        (None, None) => String::new(),
    };
    let date = if w.date.is_empty() {
        String::new()
    } else {
        format!(", {}", w.date)
    };
    let year = cfg.year();
    ImageMeta {
        title: w.scene.clone(),
        creator: format!("{artist} ({dates})"),
        date_created: w.date.clone(),
        description: format!(
            "{}. {} by {artist}{date}. {illustrates} \u{201c}{luke}\u{201d} (Luke 1:37, KJV). Published by The Scriptures, Goroshi LLC: {}",
            w.scene,
            w.medium,
            cfg.abs(&format!("art/{}/", w.slug))
        ),
        rights: format!(
            "The artwork ({artist}{date}) is in the public domain; no copyright is claimed in it. Wikimedia Commons licence: {}. Distributed by The Scriptures, Goroshi LLC. Site text and design \u{a9} {year} Stefano Theofanous and Goroshi LLC, licensed MIT or Apache-2.0.",
            w.licence_short
        ),
        rights_short: format!(
            "Artwork public domain; no copyright claimed in it. Site \u{a9} {year} Stefano Theofanous and Goroshi LLC."
        ),
        usage_terms: format!(
            "Public-domain artwork, free to use. Suggested credit: {artist}, {}{date}. Via The Scriptures, Goroshi LLC. Source: {} ({}).",
            w.scene, w.commons_url, w.usage_terms
        ),
        credit: "The Scriptures, Goroshi LLC".to_string(),
        source: "Wikimedia Commons".to_string(),
        source_id: w.commons_file.clone(),
        web_statement: cfg.abs("about/#licences"),
        page_url: cfg.abs(&format!("art/{}/", w.slug)),
        keywords: vec![
            "Bible".to_string(),
            "King James Version".to_string(),
            artist,
            w.scene.clone(),
            "public domain".to_string(),
            "The Scriptures".to_string(),
            "Goroshi LLC".to_string(),
        ],
    }
}

/// Copies a derivative into the site under a content-addressed name.
fn copy_hashed(
    out: &Path,
    src: &Path,
    stem: &str,
    ext: &str,
    report: &mut Report,
) -> Result<String> {
    let bytes = std::fs::read(src).map_err(|e| Error::io("read", src, e))?;
    let hash = sha256_hex(&bytes);
    let rel = format!("art/img/{stem}.{}.{ext}", hash.get(..10).unwrap_or(""));
    report.image_bytes += write(out, &rel, &bytes)?;
    report.images += 1;
    Ok(rel)
}

/// The widest full scan a page offers in `srcset` (wider scans are for the zoom view).
const SRCSET_FULL_MAX: u32 = 3_200;

fn stamp_images(
    inp: &Inputs,
    cfg: &SiteConfig,
    out: &Path,
    report: &mut Report,
) -> Result<HashMap<String, Served>> {
    let mut served = HashMap::new();
    for w in &inp.art.works {
        let m = image_meta(inp, cfg, w);
        let mut s = Served::default();
        let stamp = |dir: &Path, file: &str, report: &mut Report| -> Result<String> {
            let src = dir.join(file);
            let raw = std::fs::read(&src).map_err(|e| Error::io("read", &src, e))?;
            let bytes = meta::embed(&raw, &m)?;
            let hash = sha256_hex(&bytes);
            let stem = file.trim_end_matches(".jpg");
            let rel = format!("art/img/{stem}.{}.jpg", hash.get(..10).unwrap_or(""));
            report.image_bytes += write(out, &rel, &bytes)?;
            report.images += 1;
            Ok(rel)
        };
        let record = inp.crisp.as_ref().and_then(|c| c.works.get(&w.slug));
        if let (Some(record), Edition::Desktop) = (record, cfg.edition) {
            for (v, zoom) in desktop_variants(&record.variants) {
                let jpg = stamp(&inp.crisp_dir, &v.jpg, report)?;
                if zoom {
                    s.full = Some((v.width, v.height, jpg.clone(), String::new(), String::new()));
                }
                s.variants.push((v.width, v.height, jpg));
            }
        } else if let Some(record) = record {
            for v in &record.variants {
                let jpg = stamp(&inp.crisp_dir, &v.jpg, report)?;
                let stem = v.jpg.trim_end_matches(".jpg");
                let webp = copy_hashed(out, &inp.crisp_dir.join(&v.webp), stem, "webp", report)?;
                let avif = copy_hashed(out, &inp.crisp_dir.join(&v.avif), stem, "avif", report)?;
                if v.full {
                    s.full = Some((v.width, v.height, jpg.clone(), webp.clone(), avif.clone()));
                }
                if !v.full || v.width <= SRCSET_FULL_MAX {
                    s.variants.push((v.width, v.height, jpg));
                    s.webp.push((v.width, v.height, webp));
                    s.avif.push((v.width, v.height, avif));
                }
            }
        } else {
            for v in &w.variants {
                s.variants
                    .push((v.width, v.height, stamp(&inp.derived, &v.file, report)?));
            }
        }
        // The social card is for links shared on the web; the desktop app has no use for it.
        if let (Some(og), Edition::Web) = (&w.og, cfg.edition) {
            s.og = Some((og.width, og.height, stamp(&inp.derived, &og.file, report)?));
        }
        served.insert(w.slug.clone(), s);
    }
    Ok(served)
}

/// Picks the home page's featured work: the Annunciation (Luke 1, where Luke 1:37
/// stands), Doré's first, else any work at Luke 1, else the first work.
fn featured(art: &Manifest) -> Option<&crate::art::Artwork> {
    let at_luke_1 =
        |w: &&crate::art::Artwork| w.start().is_some_and(|s| s.book == 41 && s.chapter == 1);
    art.works
        .iter()
        .filter(at_luke_1)
        .find(|w| w.artist_key == "dore")
        .or_else(|| art.works.iter().find(at_luke_1))
        .or_else(|| art.works.first())
}

/// Writes pages and remembers what goes in the sitemap.
struct Pages<'o> {
    out: &'o Path,
    report: Report,
    entries: Vec<UrlEntry>,
}

impl Pages<'_> {
    /// Writes `rel_dir/index.html` (or a named `.html` file, kept out of the sitemap).
    fn page(&mut self, rel_dir: &str, html: &str, images: Vec<String>) -> Result<()> {
        let is_file = Path::new(rel_dir)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("html"));
        let file = if is_file {
            rel_dir.to_string()
        } else {
            format!("{rel_dir}index.html")
        };
        let n = write(self.out, &file, html.as_bytes())?;
        self.report.pages += 1;
        self.report.html_bytes += n;
        if n > self.report.largest_page.1 {
            self.report.largest_page = (file, n);
        }
        if !is_file {
            self.entries.push(UrlEntry {
                path: rel_dir.to_string(),
                images,
            });
        }
        Ok(())
    }
}

fn largest_abs(ctx: &Ctx<'_>, slug: &str) -> Vec<String> {
    ctx.served
        .get(slug)
        .and_then(|s| s.variants.last())
        .map(|(_, _, p)| vec![ctx.cfg.abs(p)])
        .unwrap_or_default()
}

fn write_scripture(ctx: &Ctx<'_>, pages: &mut Pages<'_>) -> Result<()> {
    for (bi, info) in canon::BOOKS.iter().enumerate() {
        let imgs = ctx
            .art
            .in_book(bi)
            .iter()
            .flat_map(|w| largest_abs(ctx, &w.slug))
            .collect();
        pages.page(&format!("{}/", info.slug), &render::book(ctx, bi), imgs)?;
        pages.report.books += 1;
        for c in 1..=ctx.bible.chapter_count(bi) {
            let imgs = ctx
                .art
                .in_chapter(bi, c)
                .iter()
                .flat_map(|w| largest_abs(ctx, &w.slug))
                .collect();
            pages.page(
                &format!("{}/{c}/", info.slug),
                &render::chapter(ctx, bi, c),
                imgs,
            )?;
            pages.report.chapters += 1;
        }
    }
    Ok(())
}

fn write_art(ctx: &Ctx<'_>, pages: &mut Pages<'_>) -> Result<()> {
    let all = ctx
        .art
        .works
        .iter()
        .flat_map(|w| largest_abs(ctx, &w.slug))
        .collect();
    pages.page("art/", &render::art_index(ctx), all)?;
    pages.page("art/browse/", &render::art_directory(ctx), Vec::new())?;
    let ordered = crate::gallery::ordered(&ctx.art.works);
    if ordered.len() != ctx.art.works.len() {
        return Err(Error::Data(format!(
            "{} admitted plate(s) have no place in the canonical order",
            ctx.art.works.len() - ordered.len()
        )));
    }
    for (i, (w, _)) in ordered.iter().enumerate() {
        let (prev, next) = crate::gallery::neighbours(&ordered, i);
        pages.page(
            &format!("art/{}/", w.slug),
            &render::art_page(ctx, w, prev.map(|p| p.0), next.map(|n| n.0)),
            largest_abs(ctx, &w.slug),
        )?;
        pages.report.art_pages += 1;
    }
    Ok(())
}

/// Content-addressed site paths for the shipped font files, recorded in the config so
/// the stylesheet can name them.
fn font_paths<'f>(
    fonts: &'f [(String, Vec<u8>)],
    cfg: &mut SiteConfig,
) -> Vec<(String, &'f Vec<u8>)> {
    let mut font_files = Vec::new();
    for (name, bytes) in fonts {
        let is_woff = Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("woff"));
        let rel = if is_woff {
            let hash = sha256_hex(bytes);
            format!(
                "fonts/{}.{}.woff",
                name.trim_end_matches(".woff"),
                hash.get(..10).unwrap_or("")
            )
        } else {
            format!("fonts/{name}")
        };
        if name == "EBGaramond-Text.woff" {
            cfg.font_regular = Some(rel.clone());
        } else if name == "EBGaramond-Italic-Text.woff" {
            cfg.font_italic = Some(rel.clone());
        }
        font_files.push((rel, bytes));
    }
    font_files
}

/// Builds the whole site into `out`.
///
/// # Errors
/// Fails when inputs are missing or the output cannot be written.
pub fn build(data: &Path, out: &Path, cfg: &mut SiteConfig) -> Result<Report> {
    let inp = Inputs::load(data)?;
    cfg.has_icon = true;
    let font_files = font_paths(&inp.fonts, cfg);
    cfg.discovery_version = crate::discovery::asset_version(data, cfg)?;
    prepare_out(out)?;
    let mut pages = Pages {
        out,
        report: Report::default(),
        entries: Vec::new(),
    };
    let served = stamp_images(&inp, cfg, out, &mut pages.report)?;
    for (rel, bytes) in &font_files {
        write(out, rel, bytes)?;
    }
    let ctx = Ctx {
        cfg,
        bible: &inp.bible,
        xrefs: &inp.xrefs,
        red: &inp.red,
        art: &inp.art,
        served: &served,
        lect: inp.lect.as_ref(),
        icon: inp.icon_record.as_ref(),
        psalm: inp.psalm.as_ref(),
        wasm: inp.wasm.as_deref(),
    };
    let feat = featured(&inp.art);
    let feat_imgs = feat.map(|w| largest_abs(&ctx, &w.slug)).unwrap_or_default();
    pages.page("", &render::home(&ctx, feat), feat_imgs)?;
    write_scripture(&ctx, &mut pages)?;
    write_art(&ctx, &mut pages)?;
    let discovery = crate::discovery::catalogue(&ctx)?;
    write(
        out,
        "discovery/catalogue.json",
        &serde_json::to_vec(&discovery).map_err(|e| Error::json("discovery catalogue", e))?,
    )?;
    write(
        out,
        "discovery/ui.js",
        crate::discovery::BROWSER_GLUE.as_bytes(),
    )?;
    if let Some(assets) = &cfg.search_assets {
        for name in ["discovery.js", "discovery_bg.wasm"] {
            let source = assets.join(name);
            let bytes = std::fs::read(&source)
                .map_err(|e| Error::io("read discovery asset", &source, e))?;
            write(out, &format!("discovery/{name}"), &bytes)?;
        }
    }
    pages.page("search/", &crate::discovery::page(&ctx, false), Vec::new())?;
    pages.page(
        "red-letter/",
        &crate::pages::red_letter_index(&ctx),
        Vec::new(),
    )?;
    for (book, _) in crate::pages::red_books(&ctx) {
        let slug = canon::book(book).map_or("", |b| b.slug);
        pages.page(
            &format!("red-letter/{slug}/"),
            &crate::pages::red_letter_book(&ctx, book),
            Vec::new(),
        )?;
    }
    pages.page("prayer/", &crate::pages::prayer(&ctx), Vec::new())?;
    pages.page("profile/", &crate::pages::profile(&ctx)?, Vec::new())?;
    write(
        out,
        "discovery/reader.js",
        crate::discovery::READER_GLUE.as_bytes(),
    )?;
    pages.page("parishes/", &crate::discovery::parishes(&ctx), Vec::new())?;
    pages.page("lectionary/", &render::lectionary(&ctx), Vec::new())?;
    pages.page("about/", &render::about(&ctx), Vec::new())?;
    pages.page("404.html", &render::not_found(&ctx), Vec::new())?;
    pages.report.sitemap_urls = pages.entries.len();
    write(
        out,
        "sitemap.xml",
        seo::sitemap(cfg, &pages.entries).as_bytes(),
    )?;
    write(out, "robots.txt", seo::robots(cfg).as_bytes())?;
    let extra: Vec<String> = inp
        .wasm
        .as_deref()
        .map(crate::features::paschalion_script)
        .into_iter()
        .collect();
    write(
        out,
        &crate::html::script_asset(crate::html::JS),
        crate::html::JS.as_bytes(),
    )?;
    for script in &extra {
        write(out, &crate::html::script_asset(script), script.as_bytes())?;
    }
    write(out, "_headers", seo::headers(cfg, &extra).as_bytes())?;
    write_icon(out)?;
    Ok(pages.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_base_url_prefixes_every_link() {
        let cfg = SiteConfig::new("https://goroshillc.com/scriptures", "2026-10-12");
        let cfg = cfg.ok();
        assert_eq!(
            cfg.as_ref().map(|c| c.href("john/3/")),
            Some("/scriptures/john/3/".to_string())
        );
        assert_eq!(
            cfg.as_ref().map(|c| c.abs("")),
            Some("https://goroshillc.com/scriptures/".to_string())
        );
    }

    #[test]
    fn the_icon_link_is_content_addressed_so_no_cache_shows_an_old_icon() {
        let cfg = SiteConfig::new("https://scriptures.goroshillc.com/", "2026-10-19").ok();
        let href = cfg.map(|mut c| {
            c.has_icon = true;
            c.icon_href()
        });
        let hash = sha256_hex(chi_rho::favicon().as_bytes());
        let name = format!("favicon-{}.svg", hash.get(..16).unwrap_or_default());
        assert_eq!(icon_file(), name);
        assert_eq!(href, Some(format!("/{name}")));
    }

    #[test]
    fn a_subdomain_base_url_links_from_the_root() {
        let cfg = SiteConfig::new("https://scriptures.goroshillc.com/", "2026-10-12").ok();
        assert_eq!(cfg.as_ref().map(|c| c.href("")), Some("/".to_string()));
        assert_eq!(
            cfg.map(|c| c.abs("sitemap.xml")),
            Some("https://scriptures.goroshillc.com/sitemap.xml".to_string())
        );
    }

    fn variant(width: u32, full: bool) -> crate::crisp::CrispVariant {
        crate::crisp::CrispVariant {
            width,
            height: width,
            jpg: format!("w-{width}.jpg"),
            webp: format!("w-{width}.webp"),
            avif: format!("w-{width}.avif"),
            full,
        }
    }

    #[test]
    fn the_desktop_edition_keeps_three_jpeg_widths_and_a_zoom_no_wider_than_1920() {
        let all: Vec<_> = [480, 720, 960, 1440, 1920, 2317]
            .into_iter()
            .map(|w| variant(w, w == 2317))
            .collect();
        let picked: Vec<(u32, bool)> = desktop_variants(&all)
            .into_iter()
            .map(|(v, zoom)| (v.width, zoom))
            .collect();
        assert_eq!(picked, vec![(480, false), (960, false), (1920, true)]);
    }

    #[test]
    fn a_small_original_is_its_own_desktop_zoom_and_never_listed_twice() {
        let small = vec![variant(480, false), variant(561, true)];
        let picked: Vec<(u32, bool)> = desktop_variants(&small)
            .into_iter()
            .map(|(v, zoom)| (v.width, zoom))
            .collect();
        assert_eq!(picked, vec![(480, false), (561, true)]);
        let exact = vec![variant(480, false), variant(960, true)];
        let picked: Vec<(u32, bool)> = desktop_variants(&exact)
            .into_iter()
            .map(|(v, zoom)| (v.width, zoom))
            .collect();
        assert_eq!(picked, vec![(480, false), (960, true)]);
    }

    #[test]
    fn editions_parse_by_name_and_refuse_anything_else() {
        assert_eq!(Edition::parse("web").ok(), Some(Edition::Web));
        assert_eq!(Edition::parse("desktop").ok(), Some(Edition::Desktop));
        assert!(Edition::parse("mobile").is_err());
    }
}
