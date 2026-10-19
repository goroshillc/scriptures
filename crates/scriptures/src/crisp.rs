//! The sharpest real scan a screen can use: Commons originals, resized and re-encoded.
//!
//! > *"For now we see through a glass, darkly; but then face to face: now I know in part;
//! > but then shall I know even as also I am known."* - 1 Corinthians 13:12, KJV
//!
//! WHY originals: the Doré maximum build resized from 1920-pixel Commons renditions, so a
//! retina screen or the zoom view could never see more of the engraving than that. This
//! module downloads each admitted file's ORIGINAL upload, proves it is the very file that
//! was reviewed (its sha1 must equal the sha1 Commons recorded for the reviewed file), and
//! writes every display width up to 2560 pixels and the full scan (to [`FULL_MAX`]) as
//! AVIF, WebP and JPEG.
//!
//! The master-art rule binds every step: pixels are only resized (Lanczos3) and
//! re-encoded. Nothing is cropped, recoloured, sharpened or redrawn; an embedded colour
//! profile is carried into every derivative so the colours are the scan's own.

use crate::art::{Artwork, Exclusions, Manifest, Review};
use crate::{Error, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageEncoder, ImageReader};
use serde::{Deserialize, Serialize};
use sha1::Digest as _;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Display widths below the full scan, narrowest first.
pub const WIDTHS: [u32; 6] = [480, 720, 960, 1440, 1920, 2560];
/// The widest full-scan derivative. A few whole-page scans run past 11,000 pixels; 6,000
/// keeps every engraved line at zoom and every file inside the host's 16 MiB limit.
pub const FULL_MAX: u32 = 6_000;
/// JPEG quality (libjpeg-turbo `cjpeg`, optimized, progressive).
pub const JPEG_QUALITY: u8 = 86;
/// WebP quality (`cwebp`, method 6, sharp YUV).
pub const WEBP_QUALITY: u8 = 86;
/// AVIF quality (`avifenc`, libaom).
pub const AVIF_QUALITY: u8 = 68;

/// One display width in three formats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrispVariant {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// JPEG file name in the crisp directory.
    pub jpg: String,
    /// WebP file name.
    pub webp: String,
    /// AVIF file name.
    pub avif: String,
    /// True for the full-scan derivative (the zoom view's image).
    pub full: bool,
}

/// The original a work's crisp derivatives come from, and the derivatives.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrispWork {
    /// The original's URL on upload.wikimedia.org.
    pub url: String,
    /// The Commons sha1 the original matched.
    pub sha1: String,
    /// Original pixel width.
    pub width: u32,
    /// Original pixel height.
    pub height: u32,
    /// Original byte count.
    pub bytes: u64,
    /// Whether the original carried an ICC colour profile (carried into every derivative).
    pub icc: bool,
    /// Derivatives, narrowest first; the last is the full scan.
    pub variants: Vec<CrispVariant>,
}

/// `data/art-cache/crisp/crisp.json`: every admitted work's crisp record by slug, plus
/// the exact encoder versions used.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrispManifest {
    /// `cjpeg -version`, `cwebp -version`, `avifenc --version` first lines.
    pub encoders: Vec<String>,
    /// Works by slug.
    pub works: BTreeMap<String, CrispWork>,
}

impl CrispManifest {
    /// Loads the record, or `None` when no crisp derivation has been run.
    ///
    /// # Errors
    /// Fails when the file exists but cannot be read.
    pub fn load(data: &Path) -> Result<Option<Self>> {
        let path = manifest_path(data);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| Error::io("read", &path, e))?;
        serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| Error::json("crisp.json", e))
    }
}

/// The crisp directory inside a data directory.
#[must_use]
pub fn crisp_dir(data: &Path) -> PathBuf {
    data.join("art-cache").join("crisp")
}

pub(crate) fn originals_dir(data: &Path) -> PathBuf {
    data.join("art-cache").join("originals")
}

fn manifest_path(data: &Path) -> PathBuf {
    crisp_dir(data).join("crisp.json")
}

/// Admitted works exactly as the build sees them: the manifest, less the AG-CHILD list,
/// less every work whose review verdict is not ADMIT.
///
/// # Errors
/// Fails when an input is missing or a work has no verdict.
pub fn admitted(data: &Path) -> Result<Vec<Artwork>> {
    let mut art = Manifest::load(&data.join("art_manifest.json"))?;
    art.apply(&Exclusions::load(&data.join("ag_child_removed.tsv"))?);
    art.apply_review(&Review::load(&data.join("content_review.tsv"))?)?;
    Ok(art.works)
}

/// The original upload's URL, from the Commons URL the master came from.
///
/// That is a rendition (`.../commons/thumb/d/d2/NAME/1920px-NAME?...` gives
/// `upload.wikimedia.org/.../d/d2/NAME`), or for a master narrower than 1920 pixels the
/// original itself.
#[must_use]
pub fn original_url(rendition: &str) -> Option<String> {
    let path = rendition.split('?').next()?;
    // A master narrower than 1920 pixels was downloaded as the original itself.
    let file_path = if let Some((_, after)) = path.split_once("/wikipedia/commons/thumb/") {
        after.rsplit_once('/')?.0
    } else {
        path.strip_prefix("https://upload.wikimedia.org/wikipedia/commons/")?
    };
    let mut parts = file_path.splitn(3, '/');
    let (a, ab, name) = (parts.next()?, parts.next()?, parts.next()?);
    let hex = |s: &str| s.chars().all(|c| c.is_ascii_hexdigit());
    if a.len() != 1 || ab.len() != 2 || !hex(a) || !hex(ab) || name.is_empty() || name.contains('/')
    {
        return None;
    }
    Some(format!(
        "https://upload.wikimedia.org/wikipedia/commons/{a}/{ab}/{name}"
    ))
}

pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    sha1::Sha1::digest(bytes)
        .iter()
        .fold(String::with_capacity(40), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What [`fetch_originals`] did.
#[derive(Debug, Default)]
pub struct FetchReport {
    /// Downloaded and verified this run.
    pub fetched: usize,
    /// Already on disk with the right sha1.
    pub kept: usize,
    /// Bytes downloaded.
    pub bytes: u64,
}

/// Downloads every admitted work's original, refusing any whose sha1 differs from the
/// sha1 Commons recorded for the reviewed file, pausing `gap` between downloads.
///
/// # Errors
/// Fails on a network error or any sha1 mismatch (nothing unverified is kept).
pub fn fetch_originals(data: &Path, gap: std::time::Duration) -> Result<FetchReport> {
    let dir = originals_dir(data);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io("create dir", &dir, e))?;
    let mut report = FetchReport::default();
    let works = admitted(data)?;
    let total = works.len();
    for (n, w) in works.iter().enumerate() {
        let path = dir.join(format!("{}.jpg", w.slug));
        if path.exists() {
            let raw = std::fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
            if sha1_hex(&raw) == w.sha1 {
                report.kept += 1;
                continue;
            }
        }
        let master = w
            .master
            .as_ref()
            .ok_or_else(|| Error::Data(format!("{}: no master record", w.slug)))?;
        let url = original_url(&master.url)
            .ok_or_else(|| Error::Data(format!("{}: cannot derive original URL", w.slug)))?;
        std::thread::sleep(gap);
        let tmp = dir.join(format!(".{}.part", w.slug));
        download(&url, &tmp)?;
        let bytes = std::fs::read(&tmp).map_err(|e| Error::io("read", &tmp, e))?;
        let got = sha1_hex(&bytes);
        if got != w.sha1 {
            std::fs::remove_file(&tmp).map_err(|e| Error::io("remove", &tmp, e))?;
            return Err(Error::Data(format!(
                "{}: original sha1 {got} is not the reviewed file's {}",
                w.slug, w.sha1
            )));
        }
        std::fs::rename(&tmp, &path).map_err(|e| Error::io("rename", &path, e))?;
        report.fetched += 1;
        report.bytes += u64::try_from(bytes.len()).unwrap_or(0);
        eprintln!(
            "  [{}/{total}] {} ({} bytes, sha1 ok)",
            n + 1,
            w.slug,
            bytes.len()
        );
    }
    Ok(report)
}

/// Downloads one original with `curl`.
///
/// WHY curl and not [`crate::net::Net`]: measured 2026-10-19, the builder's HTTP/1.1 client timed out
/// after 90 seconds on a 6 MB original that curl (HTTP/2) fetched in 0.5 seconds. curl
/// keeps the same identifying User-Agent, retries 429 and 5xx with back-off, honours
/// `Retry-After`, and the caller still verifies every byte by sha1.
fn download(url: &str, out: &Path) -> Result<()> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--retry",
            "5",
            "--retry-delay",
            "5",
            "--max-time",
            "900",
            "--max-filesize",
            "67108864",
            "--user-agent",
            crate::net::USER_AGENT,
            "--output",
        ])
        .arg(out)
        .arg(url)
        .output()
        .map_err(|e| Error::io("run curl", out, e))?;
    if !output.status.success() {
        return Err(Error::Http {
            url: url.to_string(),
            reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

/// The widths written for an original `w0` pixels wide: every display width narrower
/// than the scan, then the full scan (capped at [`FULL_MAX`]).
#[must_use]
pub fn widths_for(w0: u32) -> Vec<u32> {
    let full = w0.min(FULL_MAX);
    let mut out: Vec<u32> = WIDTHS.iter().copied().filter(|w| *w < full).collect();
    out.push(full);
    out
}

/// `h0 * w / w0`, rounded to the nearest pixel, never zero.
#[must_use]
pub fn scaled_height(w0: u32, h0: u32, w: u32) -> u32 {
    let num = u64::from(h0) * u64::from(w) * 2 + u64::from(w0.max(1));
    let h = num / (2 * u64::from(w0.max(1)));
    u32::try_from(h.max(1)).unwrap_or(u32::MAX)
}

pub(crate) fn img_err(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::Image {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

/// Decodes an original, returning its pixels and its ICC profile when it has one.
pub(crate) fn open_with_icc(path: &Path) -> Result<(DynamicImage, Option<Vec<u8>>)> {
    let mut reader = ImageReader::open(path).map_err(|e| img_err(path, e))?;
    reader.no_limits();
    let mut decoder = reader
        .with_guessed_format()
        .map_err(|e| img_err(path, e))?
        .into_decoder()
        .map_err(|e| img_err(path, e))?;
    let icc = decoder.icc_profile().map_err(|e| img_err(path, e))?;
    let img = DynamicImage::from_decoder(decoder).map_err(|e| img_err(path, e))?;
    Ok((img, icc))
}

/// Greyscale stays one channel; everything else becomes 8-bit RGB. No pixel changes.
pub(crate) fn flatten(img: DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageLuma8(_) => img,
        DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_) => DynamicImage::ImageLuma8(img.to_luma8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    }
}

/// Raw PGM/PPM for `cjpeg`.
pub(crate) fn write_pnm(img: &DynamicImage, path: &Path) -> Result<()> {
    let (w, h) = img.dimensions();
    let (magic, bytes) = match img {
        DynamicImage::ImageLuma8(g) => ("P5", g.as_raw().as_slice()),
        DynamicImage::ImageRgb8(c) => ("P6", c.as_raw().as_slice()),
        _ => return Err(img_err(path, "unexpected pixel format")),
    };
    let mut out = format!("{magic}\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(bytes);
    std::fs::write(path, out).map_err(|e| Error::io("write", path, e))
}

/// PNG (with the ICC profile) for `cwebp` and `avifenc`.
fn write_png(img: &DynamicImage, path: &Path, icc: Option<&[u8]>) -> Result<()> {
    let file = std::fs::File::create(path).map_err(|e| Error::io("create", path, e))?;
    let mut encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    );
    if let Some(profile) = icc {
        encoder
            .set_icc_profile(profile.to_vec())
            .map_err(|e| img_err(path, e))?;
    }
    let (w, h) = img.dimensions();
    let colour = img.color().into();
    encoder
        .write_image(img.as_bytes(), w, h, colour)
        .map_err(|e| img_err(path, e))
}

pub(crate) fn run(tool: &str, args: &[&std::ffi::OsStr], out: &Path) -> Result<()> {
    let status = Command::new(tool)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| Error::io("run encoder", out, e))?;
    if !status.status.success() || !out.is_file() {
        return Err(img_err(
            out,
            format!(
                "{tool} failed: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            ),
        ));
    }
    Ok(())
}

/// The first line each encoder prints about itself, recorded with the derivatives.
#[must_use]
pub fn encoder_versions() -> Vec<String> {
    let first = |tool: &str, arg: &str| {
        Command::new(tool).arg(arg).output().ok().map_or_else(
            || format!("{tool}: missing"),
            |o| {
                let text = if o.stdout.is_empty() {
                    o.stderr
                } else {
                    o.stdout
                };
                format!(
                    "{tool}: {}",
                    String::from_utf8_lossy(&text)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                )
            },
        )
    };
    vec![
        first("cjpeg", "-version"),
        first("cwebp", "-version"),
        first("avifenc", "--version"),
    ]
}

/// Writes one work's crisp derivatives, reusing any already written from this original.
///
/// # Errors
/// Fails when the original cannot be decoded or an encoder fails.
#[allow(clippy::too_many_lines)]
pub fn derive_work(data: &Path, w: &Artwork) -> Result<CrispWork> {
    let original = originals_dir(data).join(format!("{}.jpg", w.slug));
    let dir = crisp_dir(data);
    let raw = std::fs::read(&original).map_err(|e| Error::io("read", &original, e))?;
    if sha1_hex(&raw) != w.sha1 {
        return Err(Error::Data(format!(
            "{}: the original on disk is not the reviewed file",
            w.slug
        )));
    }
    let (w0, h0) = image::image_dimensions(&original).map_err(|e| img_err(&original, e))?;
    let names = |width: u32| {
        let stem = format!("{}-{width}", w.slug);
        (
            format!("{stem}.jpg"),
            format!("{stem}.webp"),
            format!("{stem}.avif"),
        )
    };
    let widths = widths_for(w0);
    let full = widths.last().copied().unwrap_or(w0);
    let make_record = |icc: bool| CrispWork {
        url: w
            .master
            .as_ref()
            .and_then(|m| original_url(&m.url))
            .unwrap_or_default(),
        sha1: w.sha1.clone(),
        width: w0,
        height: h0,
        bytes: u64::try_from(raw.len()).unwrap_or(0),
        icc,
        variants: widths
            .iter()
            .map(|width| {
                let (jpg, webp, avif) = names(*width);
                CrispVariant {
                    width: *width,
                    height: if *width == w0 {
                        h0
                    } else {
                        scaled_height(w0, h0, *width)
                    },
                    jpg,
                    webp,
                    avif,
                    full: *width == full,
                }
            })
            .collect(),
    };
    let done = widths.iter().all(|width| {
        let (a, b, c) = names(*width);
        <[String; 3]>::from((a, b, c))
            .iter()
            .all(|f| dir.join(f).is_file())
    });
    if done {
        let marker = dir.join(format!("{}.icc", w.slug));
        return Ok(make_record(marker.exists()));
    }
    let (img, icc) = open_with_icc(&original)?;
    let img = flatten(img);
    let grey = matches!(img, DynamicImage::ImageLuma8(_));
    let icc_file = dir.join(format!("{}.icc", w.slug));
    if let Some(profile) = &icc {
        std::fs::write(&icc_file, profile).map_err(|e| Error::io("write", &icc_file, e))?;
    }
    for width in &widths {
        let height = if *width == w0 {
            h0
        } else {
            scaled_height(w0, h0, *width)
        };
        let sized = if *width == w0 {
            img.clone()
        } else {
            img.resize_exact(*width, height, FilterType::Lanczos3)
        };
        let (jpg, webp, avif) = names(*width);
        let pnm = dir.join(format!(".{}-{width}.pnm", w.slug));
        let png = dir.join(format!(".{}-{width}.png", w.slug));
        write_pnm(&sized, &pnm)?;
        write_png(&sized, &png, icc.as_deref())?;
        let quality = JPEG_QUALITY.to_string();
        let jpg_path = dir.join(&jpg);
        let mut cjpeg: Vec<&std::ffi::OsStr> = vec![
            "-quality".as_ref(),
            quality.as_ref(),
            "-optimize".as_ref(),
            "-progressive".as_ref(),
        ];
        if icc.is_some() {
            cjpeg.push("-icc".as_ref());
            cjpeg.push(icc_file.as_os_str());
        }
        cjpeg.push("-outfile".as_ref());
        cjpeg.push(jpg_path.as_os_str());
        cjpeg.push(pnm.as_os_str());
        run("cjpeg", &cjpeg, &jpg_path)?;
        let wq = WEBP_QUALITY.to_string();
        let webp_path = dir.join(&webp);
        run(
            "cwebp",
            &[
                "-quiet".as_ref(),
                "-q".as_ref(),
                wq.as_ref(),
                "-m".as_ref(),
                "6".as_ref(),
                "-sharp_yuv".as_ref(),
                "-metadata".as_ref(),
                "icc".as_ref(),
                png.as_os_str(),
                "-o".as_ref(),
                webp_path.as_os_str(),
            ],
            &webp_path,
        )?;
        let aq = AVIF_QUALITY.to_string();
        let avif_path = dir.join(&avif);
        run(
            "avifenc",
            &[
                "-q".as_ref(),
                aq.as_ref(),
                "-s".as_ref(),
                "6".as_ref(),
                "-j".as_ref(),
                "4".as_ref(),
                "--yuv".as_ref(),
                if grey { "400" } else { "420" }.as_ref(),
                png.as_os_str(),
                avif_path.as_os_str(),
            ],
            &avif_path,
        )?;
        for temp in [&pnm, &png] {
            std::fs::remove_file(temp).map_err(|e| Error::io("remove temp", temp, e))?;
        }
    }
    Ok(make_record(icc.is_some()))
}

/// Derives every admitted work on `threads` worker threads and writes `crisp.json`.
///
/// # Errors
/// Fails when any work fails; works already finished stay on disk for the next run.
pub fn derive_all(data: &Path, threads: usize) -> Result<CrispManifest> {
    let dir = crisp_dir(data);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io("create dir", &dir, e))?;
    let works = admitted(data)?;
    let total = works.len();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(w) = works.get(i) else { break };
                    let result = derive_work(data, w);
                    eprintln!(
                        "  [{}/{total}] {} {}",
                        i + 1,
                        w.slug,
                        if result.is_ok() { "ok" } else { "FAILED" }
                    );
                    if let Ok(mut all) = results.lock() {
                        all.push((w.slug.clone(), result));
                    }
                }
            });
        }
    });
    let mut manifest = CrispManifest {
        encoders: encoder_versions(),
        works: BTreeMap::new(),
    };
    let collected = results
        .into_inner()
        .map_err(|_| Error::Data("a derivation thread panicked".into()))?;
    for (slug, result) in collected {
        manifest.works.insert(slug, result?);
    }
    let path = manifest_path(data);
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| Error::json("crisp.json", e))?;
    std::fs::write(&path, json).map_err(|e| Error::io("write", &path, e))?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_url_is_read_from_the_rendition_url() {
        assert_eq!(
            original_url(
                "https://thumb.wikimedia.org/wikipedia/commons/thumb/d/d2/001.The_Creation_of_Light.jpg/1920px-001.The_Creation_of_Light.jpg?utm_source=commons.wikimedia.org"
            ),
            Some(
                "https://upload.wikimedia.org/wikipedia/commons/d/d2/001.The_Creation_of_Light.jpg"
                    .to_string()
            )
        );
        assert_eq!(
            original_url(
                "https://upload.wikimedia.org/wikipedia/commons/d/dd/DoreDeathofAnanias.jpg?utm_content=original"
            ),
            Some(
                "https://upload.wikimedia.org/wikipedia/commons/d/dd/DoreDeathofAnanias.jpg"
                    .to_string()
            )
        );
        assert_eq!(original_url("https://example.org/x.jpg"), None);
        assert_eq!(
            original_url(
                "https://upload.wikimedia.org/wikipedia/commons/thumb/zz/d2/a.jpg/1px-a.jpg"
            ),
            None
        );
    }

    #[test]
    fn widths_climb_to_the_full_scan_and_never_past_it() {
        assert_eq!(widths_for(2317), vec![480, 720, 960, 1440, 1920, 2317]);
        assert_eq!(
            widths_for(5874),
            vec![480, 720, 960, 1440, 1920, 2560, 5874]
        );
        assert_eq!(
            widths_for(11_171),
            vec![480, 720, 960, 1440, 1920, 2560, 6000]
        );
        assert_eq!(widths_for(1000), vec![480, 720, 960, 1000]);
        assert_eq!(widths_for(561), vec![480, 561]);
    }

    #[test]
    fn heights_keep_the_true_proportions() {
        assert_eq!(scaled_height(2317, 2920, 960), 1210);
        assert_eq!(scaled_height(2317, 2920, 2317), 2920);
        assert_eq!(scaled_height(1, 1, 1), 1);
    }

    #[test]
    fn sha1_matches_the_published_test_vector() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
