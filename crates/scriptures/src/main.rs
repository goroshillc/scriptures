//! `goroshi-scriptures` - fetch the sources, build the site, preview it.
//!
//! > *"And the rain descended, and the floods came, and the winds blew, and beat upon
//! > that house; and it fell not: for it was founded upon a rock."* - Matthew 7:25, KJV
//!
//! ```text
//! goroshi-scriptures fetch-redletter [--data DIR]
//! goroshi-scriptures fetch-art       [--data DIR]
//! goroshi-scriptures fetch-lectionary [--data DIR] [--days N]
//! goroshi-scriptures fetch-icon      [--data DIR]
//! goroshi-scriptures fetch-originals [--data DIR]   (Commons originals, sha1-verified)
//! goroshi-scriptures derive-crisp    [--data DIR] [--threads N]   (AVIF, WebP, JPEG)
//! goroshi-scriptures sources         [--data DIR]   (writes SOURCES.md)
//! goroshi-scriptures wallpapers --data DIR --out NEW_DIR [--sizes 1920x1080,...] [--threads N]
//! goroshi-scriptures desktop-files [--data DIR]   (the desktop art pack's file list)
//! goroshi-scriptures build  [--data DIR] [--out DIR] [--base-url URL] [--date YYYY-MM-DD] [--edition web|desktop]
//! goroshi-scriptures contact-sheet --data DIR --out NEW_DIST --sheet NEW_FILE.html
//! goroshi-scriptures verify [--out DIR] [--base-url URL]   (crawl: 1,189 chapters, 0 broken)
//! ```

use goroshi_scriptures::bible::Bible;
use goroshi_scriptures::commons;
use goroshi_scriptures::net::Net;
use goroshi_scriptures::redletter::{self, RedLetters};
use goroshi_scriptures::site::{self, SiteConfig};
use goroshi_scriptures::{Error, Result, lectionary};
use std::path::{Path, PathBuf};
use std::time::Duration;

const EBIBLE_URL: &str = "https://ebible.org/Scriptures/eng-kjv2006_usfm.zip";
/// Where the reader is published.
const DEFAULT_BASE: &str = "https://scriptures.goroshillc.com";

struct Args {
    cmd: String,
    data: PathBuf,
    out: PathBuf,
    base_url: String,
    date: Option<String>,
    search_assets: Option<PathBuf>,
    wasm: Option<PathBuf>,
    sheet: Option<PathBuf>,
    days: i64,
    threads: usize,
    edition: site::Edition,
    sizes: Option<Vec<(u32, u32)>>,
}

fn parse_args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or_else(|| {
        Error::Usage(
            "command required: fetch-redletter | fetch-art | fetch-lectionary | fetch-originals | derive-crisp | build | verify | wallpapers"
                .into(),
        )
    })?;
    let mut a = Args {
        cmd,
        data: PathBuf::from("data"),
        out: PathBuf::from("dist"),
        base_url: DEFAULT_BASE.to_string(),
        date: None,
        search_assets: None,
        wasm: None,
        sheet: None,
        days: 60,
        threads: 4,
        edition: site::Edition::Web,
        sizes: None,
    };
    while let Some(flag) = it.next() {
        let val = it
            .next()
            .ok_or_else(|| Error::Usage(format!("{flag} needs a value")))?;
        match flag.as_str() {
            "--data" => a.data = PathBuf::from(val),
            "--out" => a.out = PathBuf::from(val),
            "--base-url" => a.base_url = val,
            "--date" => a.date = Some(val),
            "--search-assets" => a.search_assets = Some(PathBuf::from(val)),
            "--wasm" => a.wasm = Some(PathBuf::from(val)),
            "--sheet" => a.sheet = Some(PathBuf::from(val)),
            "--edition" => a.edition = site::Edition::parse(&val)?,
            "--sizes" => a.sizes = Some(parse_sizes(&val)?),
            "--threads" => {
                a.threads = val
                    .parse()
                    .map_err(|_| Error::Usage(format!("bad thread count {val}")))?;
            }
            "--days" => {
                a.days = val
                    .parse()
                    .map_err(|_| Error::Usage(format!("bad day count {val}")))?;
            }
            _ => return Err(Error::Usage(format!("unknown flag {flag}"))),
        }
    }
    Ok(a)
}

/// Every admitted plate centred on white at common screen sizes, plus one zip per size.
fn wallpapers(a: &Args) -> Result<()> {
    let sizes = a
        .sizes
        .clone()
        .unwrap_or_else(|| goroshi_scriptures::wallpaper::SIZES.to_vec());
    let r = goroshi_scriptures::wallpaper::write_all(&a.data, &a.out, &sizes, a.threads)?;
    let zips = goroshi_scriptures::wallpaper::zip_all(&a.out, &sizes)?;
    eprintln!(
        "wallpapers: {} written ({} bytes) in {} sizes, {} left out as too small; {} zip files in {}",
        r.written,
        r.bytes,
        sizes.len(),
        r.skipped.len(),
        zips.len(),
        a.out.display()
    );
    for (size, slug) in &r.skipped {
        eprintln!("  too small for {size}: {slug}");
    }
    Ok(())
}

/// Prints, relative to `DATA/art-cache`, every file the desktop edition reads: the crisp
/// manifest and each admitted plate's desktop JPEGs (the art pack's contents).
fn desktop_files(data: &Path) -> Result<()> {
    let crisp = goroshi_scriptures::crisp::CrispManifest::load(data)?
        .ok_or_else(|| Error::Data("no crisp.json: run fetch-art.sh first".into()))?;
    let mut out = String::from("crisp/crisp.json\n");
    for w in goroshi_scriptures::crisp::admitted(data)? {
        let record = crisp
            .works
            .get(&w.slug)
            .ok_or_else(|| Error::Data(format!("{}: no crisp record", w.slug)))?;
        for (v, _) in site::desktop_variants(&record.variants) {
            out.push_str("crisp/");
            out.push_str(&v.jpg);
            out.push('\n');
        }
    }
    print!("{out}");
    Ok(())
}

/// `1920x1080,2560x1440` -> `[(1920, 1080), (2560, 1440)]`.
fn parse_sizes(list: &str) -> Result<Vec<(u32, u32)>> {
    list.split(',')
        .map(|item| {
            let (w, h) = item
                .trim()
                .split_once('x')
                .ok_or_else(|| Error::Usage(format!("bad size {item} (want WIDTHxHEIGHT)")))?;
            let num = |v: &str| {
                v.parse::<u32>()
                    .ok()
                    .filter(|n| (320..=16_384).contains(n))
                    .ok_or_else(|| Error::Usage(format!("bad size {item}")))
            };
            Ok((num(w)?, num(h)?))
        })
        .collect()
}

fn fetch_redletter(data: &Path) -> Result<()> {
    let raw_dir = data.join("raw");
    std::fs::create_dir_all(&raw_dir).map_err(|e| Error::io("create dir", &raw_dir, e))?;
    let zip_path = raw_dir.join("eng-kjv2006_usfm.zip");
    if !zip_path.exists() {
        let mut net = Net::new(Duration::from_secs(1));
        let bytes = net.get(EBIBLE_URL)?;
        std::fs::write(&zip_path, bytes).map_err(|e| Error::io("write", &zip_path, e))?;
    }
    let zip_bytes = std::fs::read(&zip_path).map_err(|e| Error::io("read", &zip_path, e))?;
    let kjv_path = data.join("kjv.json");
    let kjv_bytes = std::fs::read(&kjv_path).map_err(|e| Error::io("read", &kjv_path, e))?;
    let bible = Bible::from_json(&String::from_utf8_lossy(&kjv_bytes))?;
    let usfm = redletter::read_usfm_zip(&zip_path)?;
    let d = redletter::derive(&bible, &usfm);
    let table = RedLetters {
        source: EBIBLE_URL.to_string(),
        source_sha256: site::sha256_hex(&zip_bytes),
        kjv_sha256: site::sha256_hex(&kjv_bytes),
        usfm_red_verses: d.usfm_red_verses,
        usfm_red_words: d.usfm_red_words,
        aligned_red_words: d.aligned_red_words,
        spans: d.spans,
    };
    eprintln!(
        "red letters: {} USFM verses with \\wj, {} USFM red words, {} reader words aligned red, {} verses with spans",
        table.usfm_red_verses,
        table.usfm_red_words,
        table.aligned_red_words,
        table.spans.len()
    );
    let out = data.join("redletter.json");
    let json = serde_json::to_string(&table).map_err(|e| Error::json("redletter.json", e))?;
    std::fs::write(&out, json).map_err(|e| Error::io("write", &out, e))
}

fn fetch_art(data: &Path) -> Result<()> {
    let mut net = Net::new(Duration::from_millis(1100));
    let manifest = commons::fetch(
        &data.join("art_plan.tsv"),
        &data.join("art-cache"),
        &mut net,
    )?;
    eprintln!(
        "art: {} admitted, {} refused",
        manifest.works.len(),
        manifest.refused.len()
    );
    let out = data.join("art_manifest.json");
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| Error::json("art_manifest.json", e))?;
    std::fs::write(&out, json).map_err(|e| Error::io("write", &out, e))
}

/// Writes `SOURCES.md`: every admitted artwork with its Commons licence verbatim.
fn write_sources(data: &Path, crate_dir: &Path) -> Result<()> {
    use std::fmt::Write as _;
    let m = goroshi_scriptures::art::Manifest::load(&data.join("art_manifest.json"))?;
    let mut s = String::from(
        "# Art sources\n\nGenerated by `goroshi-scriptures sources` from `data/art_manifest.json`. Every licence below was read from the Wikimedia Commons API for that file; only `pd` and `cc0` pass the gate.\n\n| # | Work | Artist (Commons) | Date | Commons file | Licence (verbatim) | Code | Passage | Retrieved |\n|---|---|---|---|---|---|---|---|---|\n",
    );
    for (i, w) in m.works.iter().enumerate() {
        let passage = w
            .passage
            .clone()
            .or_else(|| w.other_ref.clone())
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | [{}]({}) | {} | `{}` | {} | {} |",
            i + 1,
            w.scene.replace('|', "/"),
            w.artist.replace('|', "/"),
            w.date,
            w.commons_file.replace('|', "/"),
            w.commons_url,
            w.licence_short,
            w.licence_code,
            passage,
            w.retrieved.get(..10).unwrap_or(&w.retrieved)
        );
    }
    let _ = writeln!(s, "\nRefused by the gate: {}.", m.refused.len());
    for (file, reason) in &m.refused {
        let _ = writeln!(s, "- {file}: {reason}");
    }
    let out = crate_dir.join("SOURCES.md");
    std::fs::write(&out, s).map_err(|e| Error::io("write", &out, e))
}

fn fetch_icon(data: &Path) -> Result<()> {
    let mut net = Net::new(Duration::from_secs(1));
    let r = commons::fetch_icon(&mut net, "File:Simple Labarum.svg", data)?;
    eprintln!(
        "icon: {} ({}, {})",
        r.commons_file, r.licence_short, r.artist
    );
    Ok(())
}

fn fetch_lectionary(data: &Path, days: i64) -> Result<()> {
    let mut net = Net::new(Duration::from_millis(600));
    let lect = lectionary::fetch(&mut net, 1, days)?;
    eprintln!(
        "lectionary: {} days from {}",
        lect.days.len(),
        lect.days.first().map_or("", |d| d.date.as_str())
    );
    let out = data.join("lectionary.json");
    let json =
        serde_json::to_string_pretty(&lect).map_err(|e| Error::json("lectionary.json", e))?;
    std::fs::write(&out, json).map_err(|e| Error::io("write", &out, e))
}

/// The crawl: every chapter reachable, no broken link, every page light by default, and
/// the reading loop from Revelation 22 back to Genesis 1.
fn verify(a: &Args) -> Result<()> {
    let cfg = SiteConfig::new(&a.base_url, "1970-01-01")?;
    let c = goroshi_scriptures::verify::crawl(&a.out, &cfg.base_path)?;
    eprintln!(
        "crawl from the home page: {} pages, {} books, {} chapters, {} images, {} broken links; light default and shared tokens on {} pages, {} failures; Revelation 22 -> Genesis 1 loop: {}",
        c.pages,
        c.books,
        c.chapters,
        c.images,
        c.broken.len(),
        c.themed,
        c.theme_failures.len(),
        if c.loop_ok { "ok" } else { "MISSING" }
    );
    for (page, href) in c.broken.iter().take(20) {
        eprintln!("  broken: {page} -> {href}");
    }
    for (page, reason) in c.theme_failures.iter().take(20) {
        eprintln!("  theme: {page}: {reason}");
    }
    if c.chapters == 1189
        && c.books == 66
        && c.broken.is_empty()
        && c.theme_failures.is_empty()
        && c.loop_ok
    {
        Ok(())
    } else {
        Err(Error::Data(
                    "crawl failed: missing chapters, broken links, a page not light by default, or the reading loop".into(),
                ))
    }
}

fn run() -> Result<()> {
    let a = parse_args()?;
    match a.cmd.as_str() {
        "check-search-wasm" => {
            let result = goroshi_scriptures::wasm_check::verify(&a.data)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result)
                    .map_err(|error| Error::json("WASM parity report", error))?
            );
            Ok(())
        }
        "preview-search" => {
            let cfg = SiteConfig::new(&a.base_url, "2026-10-16")?;
            goroshi_scriptures::discovery::preview(&a.data, &a.out, &cfg.base_path)
        }
        "bind-search" => {
            let input = a
                .wasm
                .ok_or_else(|| Error::Usage("bind-search needs --wasm FILE --out DIR".into()))?;
            let mut bind = wasm_bindgen_cli_support::Bindgen::new();
            bind.input_path(input)
                .out_name("discovery")
                .web(true)
                .map_err(|e| Error::Data(e.to_string()))?;
            bind.generate(&a.out)
                .map_err(|e| Error::Data(e.to_string()))
        }
        "contact-sheet" => {
            let sheet = a.sheet.ok_or_else(|| {
                Error::Usage(
                    "contact-sheet needs --data DIR --out BUILT_DIST --sheet NEW.html".into(),
                )
            })?;
            let c = goroshi_scriptures::contact::write(&a.data, &a.out, &sheet)?;
            eprintln!(
                "contact sheet: {} fetched, {} refused by the licence gate, {} admitted, {} excluded; Doré {} fetched, {} admitted, {} excluded -> {}",
                c.fetched,
                c.refused,
                c.admitted,
                c.excluded,
                c.dore.0,
                c.dore.1,
                c.dore.2,
                sheet.display()
            );
            Ok(())
        }
        "fetch-originals" => {
            let r =
                goroshi_scriptures::crisp::fetch_originals(&a.data, Duration::from_millis(1100))?;
            eprintln!(
                "originals: {} fetched ({} bytes), {} already verified; every sha1 equals the reviewed file's",
                r.fetched, r.bytes, r.kept
            );
            Ok(())
        }
        "derive-crisp" => {
            let m = goroshi_scriptures::crisp::derive_all(&a.data, a.threads)?;
            let files: usize = m.works.values().map(|w| w.variants.len() * 3).sum();
            eprintln!(
                "crisp: {} works, {files} files (AVIF, WebP, JPEG); encoders: {}",
                m.works.len(),
                m.encoders.join("; ")
            );
            Ok(())
        }
        "wallpapers" => wallpapers(&a),
        "desktop-files" => desktop_files(&a.data),
        "fetch-redletter" => fetch_redletter(&a.data),
        "fetch-art" => fetch_art(&a.data),
        "fetch-lectionary" => fetch_lectionary(&a.data, a.days),
        "fetch-icon" => fetch_icon(&a.data),
        "sources" => write_sources(&a.data, Path::new(".")),
        "build" => {
            let date = a.date.unwrap_or_else(commons::today_utc);
            let mut cfg = SiteConfig::new(&a.base_url, &date)?;
            cfg.search_assets = a.search_assets;
            cfg.edition = a.edition;
            let r = site::build(&a.data, &a.out, &mut cfg)?;
            eprintln!(
                "built {} pages ({} books, {} chapters, {} art pages), {} images, {} sitemap URLs; HTML {} bytes, images {} bytes; largest page {} ({} bytes)",
                r.pages,
                r.books,
                r.chapters,
                r.art_pages,
                r.images,
                r.sitemap_urls,
                r.html_bytes,
                r.image_bytes,
                r.largest_page.0,
                r.largest_page.1
            );
            Ok(())
        }
        "verify" => verify(&a),
        "serve" => Err(Error::Usage(
            "Static output uses the existing site host; this builder opens no server".into(),
        )),
        other => Err(Error::Usage(format!("unknown command {other}"))),
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
