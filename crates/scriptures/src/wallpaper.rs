//! Desktop wallpapers: every admitted plate centred on white at common screen sizes.
//!
//! > *"Let your light so shine before men, that they may see your good works, and glorify
//! > your Father which is in heaven."* - Matthew 5:16, KJV
//!
//! WHY its own command: the plates are public-domain master engravings, and a wallpaper
//! pack lets them be seen every day on any computer. The master-art rule binds every step
//! here exactly as in [`crate::crisp`]: each Commons original is proven to be the reviewed
//! file (sha1), then only resized (Lanczos3, never enlarged) and placed on a white canvas.
//! Nothing is cropped, recoloured, sharpened or redrawn; an embedded colour profile is kept.
//!
//! Files are numbered in the order of the Bible, so a folder sorted by name (as Omarchy's
//! background switcher sorts it) walks from Genesis to Revelation.

use crate::art::Artwork;
use crate::crisp::{flatten, img_err, open_with_icc, originals_dir, run, sha1_hex, write_pnm};
use crate::{Error, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The screen sizes written by default: Full HD, QHD, 4K UHD, and 16:10 laptops.
pub const SIZES: [(u32, u32); 4] = [(1920, 1080), (2560, 1440), (3840, 2160), (2560, 1600)];
/// The plate's largest share of the screen height, in percent (the rest is white).
pub const MAX_HEIGHT_PERCENT: u64 = 82;
/// The plate's largest share of the screen width, in percent.
pub const MAX_WIDTH_PERCENT: u64 = 90;
/// A plate smaller than this share of the screen in both directions is left out of that
/// size rather than enlarged.
pub const MIN_PERCENT: u64 = 50;
/// JPEG quality (libjpeg-turbo `cjpeg`, optimized, progressive).
pub const JPEG_QUALITY: u8 = 90;

/// Where a plate sits on one screen: its scaled size and its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// Scaled plate width.
    pub width: u32,
    /// Scaled plate height.
    pub height: u32,
    /// Left edge on the canvas.
    pub x: u32,
    /// Top edge on the canvas.
    pub y: u32,
}

fn ratio(a: u64, b: u64, c: u64) -> Option<u32> {
    // a * b / c, rounded to the nearest whole pixel.
    let num = a.checked_mul(b)?;
    u32::try_from(num.checked_add(c / 2)? / c.max(1)).ok()
}

/// Fits a `w0` by `h0` plate on a `sw` by `sh` screen inside the white margins, never
/// enlarging it, or `None` when it would fill less than [`MIN_PERCENT`] of the screen in
/// both directions.
#[must_use]
pub fn fit(w0: u32, h0: u32, sw: u32, sh: u32) -> Option<Fit> {
    let (w0, h0) = (u64::from(w0), u64::from(h0));
    let (screen_w, screen_h) = (u64::from(sw), u64::from(sh));
    if w0 == 0 || h0 == 0 {
        return None;
    }
    let box_w = screen_w * MAX_WIDTH_PERCENT / 100;
    let box_h = screen_h * MAX_HEIGHT_PERCENT / 100;
    // Height-limited when the plate is taller than the box in proportion.
    let (width, height) = if w0 * box_h <= h0 * box_w {
        let h = box_h.min(h0);
        (ratio(w0, h, h0)?, u32::try_from(h).ok()?)
    } else {
        let w = box_w.min(w0);
        (u32::try_from(w).ok()?, ratio(h0, w, w0)?)
    };
    let small = u64::from(width) * 100 < screen_w * MIN_PERCENT
        && u64::from(height) * 100 < screen_h * MIN_PERCENT;
    if small || width == 0 || height == 0 {
        return None;
    }
    Some(Fit {
        width,
        height,
        x: (sw - width.min(sw)) / 2,
        y: (sh - height.min(sh)) / 2,
    })
}

/// The file name of plate number `n` (1-based, in the order of the Bible).
#[must_use]
pub fn file_name(n: usize, slug: &str) -> String {
    format!("{n:03}-{slug}.jpg")
}

/// What one run wrote.
#[derive(Debug, Default)]
pub struct Report {
    /// Wallpapers written.
    pub written: usize,
    /// (size, slug) pairs left out because the plate was too small for that screen.
    pub skipped: Vec<(String, String)>,
    /// Bytes written (images only).
    pub bytes: u64,
}

fn canvas_for(plate: &DynamicImage, sw: u32, sh: u32, at: Fit) -> DynamicImage {
    match plate {
        DynamicImage::ImageLuma8(g) => {
            let mut c = image::GrayImage::from_pixel(sw, sh, image::Luma([255]));
            image::imageops::overlay(&mut c, g, i64::from(at.x), i64::from(at.y));
            DynamicImage::ImageLuma8(c)
        }
        other => {
            let mut c = image::RgbImage::from_pixel(sw, sh, image::Rgb([255, 255, 255]));
            image::imageops::overlay(&mut c, &other.to_rgb8(), i64::from(at.x), i64::from(at.y));
            DynamicImage::ImageRgb8(c)
        }
    }
}

fn encode(img: &DynamicImage, icc: Option<&Path>, scratch: &Path, out: &Path) -> Result<()> {
    write_pnm(img, scratch)?;
    let quality = JPEG_QUALITY.to_string();
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "-quality".as_ref(),
        quality.as_ref(),
        "-optimize".as_ref(),
        "-progressive".as_ref(),
    ];
    if let Some(profile) = icc {
        args.push("-icc".as_ref());
        args.push(profile.as_os_str());
    }
    args.push("-outfile".as_ref());
    args.push(out.as_os_str());
    args.push(scratch.as_os_str());
    run("cjpeg", &args, out)?;
    std::fs::remove_file(scratch).map_err(|e| Error::io("remove temp", scratch, e))
}

fn size_dir(out: &Path, (sw, sh): (u32, u32)) -> PathBuf {
    out.join(format!("scriptures-wallpapers-{sw}x{sh}"))
}

/// Writes every size of one plate.
fn write_plate(
    data: &Path,
    out: &Path,
    n: usize,
    w: &Artwork,
    sizes: &[(u32, u32)],
) -> Result<Report> {
    let original = originals_dir(data).join(format!("{}.jpg", w.slug));
    let raw = std::fs::read(&original).map_err(|e| Error::io("read", &original, e))?;
    if sha1_hex(&raw) != w.sha1 {
        return Err(Error::Data(format!(
            "{}: the original on disk is not the reviewed file",
            w.slug
        )));
    }
    let (img, icc) = open_with_icc(&original)?;
    let img = flatten(img);
    let (w0, h0) = img.dimensions();
    let mut report = Report::default();
    let icc_file = out.join(format!(".{}.icc", w.slug));
    if let Some(profile) = &icc {
        std::fs::write(&icc_file, profile).map_err(|e| Error::io("write", &icc_file, e))?;
    }
    for &(sw, sh) in sizes {
        let Some(at) = fit(w0, h0, sw, sh) else {
            report.skipped.push((format!("{sw}x{sh}"), w.slug.clone()));
            continue;
        };
        let plate = if (at.width, at.height) == (w0, h0) {
            img.clone()
        } else {
            img.resize_exact(at.width, at.height, FilterType::Lanczos3)
        };
        let canvas = canvas_for(&plate, sw, sh, at);
        let dir = size_dir(out, (sw, sh));
        let target = dir.join(file_name(n, &w.slug));
        let scratch = dir.join(format!(".{}.pnm", w.slug));
        encode(
            &canvas,
            icc.as_ref().map(|_| icc_file.as_path()),
            &scratch,
            &target,
        )?;
        report.bytes += std::fs::metadata(&target)
            .map_err(|e| Error::io("stat", &target, e))?
            .len();
        report.written += 1;
    }
    if icc.is_some() {
        std::fs::remove_file(&icc_file).map_err(|e| Error::io("remove temp", &icc_file, e))?;
    }
    if report.written == 0 && sizes.is_empty() {
        return Err(img_err(&original, "no sizes requested"));
    }
    Ok(report)
}

/// The credits file: every plate with its artist, date, passage, source and licence.
#[must_use]
pub fn credits(order: &[(usize, &Artwork)]) -> String {
    let mut s = String::from(
        "The Scriptures - Doré Bible wallpapers\n\
         https://github.com/goroshillc/scriptures\n\n\
         Every image is a public-domain master work from Wikimedia Commons; no copyright is\n\
         claimed in it. Each was only resized and centred on white. Free to use.\n\n",
    );
    for (n, w) in order {
        let passage = w
            .passage
            .clone()
            .or_else(|| w.other_ref.clone())
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "{}  {} - {}, {} ({})\n    {}\n    Licence: {} ({})\n",
            file_name(*n, &w.slug),
            w.scene,
            w.artist,
            w.date,
            passage,
            w.commons_url,
            w.licence_short,
            w.licence_code
        );
    }
    s
}

/// The install notes written beside each size.
#[must_use]
pub fn readme((sw, sh): (u32, u32)) -> String {
    format!(
        "The Scriptures - Doré Bible wallpapers, {sw} x {sh}\n\
         https://github.com/goroshillc/scriptures\n\n\
         Omarchy: copy the images into the background folder of the theme you use, then\n\
         press Super + Ctrl + Space (or run `omarchy theme bg next`):\n\n\
         \x20   theme=$(cat ~/.local/state/omarchy/current/theme.name)\n\
         \x20   mkdir -p ~/.config/omarchy/backgrounds/$theme\n\
         \x20   cp *.jpg ~/.config/omarchy/backgrounds/$theme/\n\n\
         The plates sit on white, so they suit Omarchy's White theme best.\n\n\
         Any other system: set any image here as the desktop background (fill or fit).\n\
         CREDITS.txt names every plate, its passage and its source.\n"
    )
}

/// Writes every size of every admitted plate into a new directory `out`.
///
/// # Errors
/// Fails when `out` already exists, an original is missing or is not the reviewed file,
/// or the encoder fails.
pub fn write_all(data: &Path, out: &Path, sizes: &[(u32, u32)], threads: usize) -> Result<Report> {
    if out.exists() {
        return Err(Error::Usage(format!(
            "{} already exists; wallpapers are written only into a new directory",
            out.display()
        )));
    }
    let works = crate::crisp::admitted(data)?;
    let order: Vec<(usize, &Artwork)> = crate::gallery::ordered(&works)
        .into_iter()
        .enumerate()
        .map(|(i, (w, _))| (i + 1, w))
        .collect();
    let notes = credits(&order);
    for &size in sizes {
        let dir = size_dir(out, size);
        std::fs::create_dir_all(&dir).map_err(|e| Error::io("create dir", &dir, e))?;
        for (name, text) in [("README.txt", readme(size)), ("CREDITS.txt", notes.clone())] {
            let path = dir.join(name);
            std::fs::write(&path, text).map_err(|e| Error::io("write", &path, e))?;
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let total = order.len();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((n, w)) = order.get(i) else { break };
                    let result = write_plate(data, out, *n, w, sizes);
                    eprintln!(
                        "  [{}/{total}] {} {}",
                        i + 1,
                        w.slug,
                        if result.is_ok() { "ok" } else { "FAILED" }
                    );
                    if let Ok(mut all) = results.lock() {
                        all.push(result);
                    }
                }
            });
        }
    });
    let mut report = Report::default();
    let collected = results
        .into_inner()
        .map_err(|_| Error::Data("a wallpaper thread panicked".into()))?;
    for result in collected {
        let one = result?;
        report.written += one.written;
        report.bytes += one.bytes;
        report.skipped.extend(one.skipped);
    }
    report.skipped.sort();
    Ok(report)
}

/// Packs each size directory under `out` into `out/<name>.zip` (stored, not deflated:
/// JPEG does not compress further), with fixed timestamps so a rebuild is byte-identical.
///
/// # Errors
/// Fails when a directory cannot be read or the archive cannot be written.
pub fn zip_all(out: &Path, sizes: &[(u32, u32)]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for &size in sizes {
        let dir = size_dir(out, size);
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::Data(format!("bad directory {}", dir.display())))?
            .to_string();
        let zip_path = out.join(format!("{name}.zip"));
        let file =
            std::fs::File::create(&zip_path).map_err(|e| Error::io("create", &zip_path, e))?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644)
            .large_file(true);
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| Error::io("read dir", &dir, e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.'))
            })
            .collect();
        entries.sort();
        let zerr = |e: zip::result::ZipError| Error::Data(format!("{}: {e}", zip_path.display()));
        for path in entries {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::Data(format!("bad file name {}", path.display())))?;
            zip.start_file(format!("{name}/{file_name}"), options)
                .map_err(zerr)?;
            let bytes = std::fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
            zip.write_all(&bytes)
                .map_err(|e| Error::io("write", &zip_path, e))?;
        }
        let mut inner = zip.finish().map_err(zerr)?;
        inner
            .flush()
            .map_err(|e| Error::io("write", &zip_path, e))?;
        written.push(zip_path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_portrait_plate_fills_82_percent_of_a_full_hd_screen_and_is_centred() {
        // Doré's Creation of Light: 2317 x 2920.
        let at = fit(2317, 2920, 1920, 1080);
        assert_eq!(
            at,
            Some(Fit {
                width: 702,
                height: 885,
                x: 609,
                y: 97
            })
        );
    }

    #[test]
    fn a_landscape_plate_is_limited_by_the_width_margin() {
        let at = fit(6000, 2000, 1920, 1080);
        assert_eq!(
            at,
            Some(Fit {
                width: 1728,
                height: 576,
                x: 96,
                y: 252
            })
        );
    }

    #[test]
    fn a_plate_is_never_enlarged_and_a_tiny_one_is_left_out() {
        // 1000 x 1300 on 4K: the box allows 1771 high, so the plate keeps its own size.
        assert_eq!(
            fit(1000, 1300, 3840, 2160),
            Some(Fit {
                width: 1000,
                height: 1300,
                x: 1420,
                y: 430
            })
        );
        assert_eq!(fit(561, 700, 3840, 2160), None);
        assert_eq!(fit(0, 700, 1920, 1080), None);
    }

    #[test]
    fn file_names_sort_in_the_order_of_the_bible() {
        assert_eq!(
            file_name(1, "the-creation-of-light"),
            "001-the-creation-of-light.jpg"
        );
        assert!(file_name(9, "z") < file_name(10, "a"));
    }

    #[test]
    fn the_install_notes_name_omarchys_own_background_folder() {
        let text = readme((3840, 2160));
        assert!(text.contains("~/.config/omarchy/backgrounds/$theme"));
        assert!(text.contains("~/.local/state/omarchy/current/theme.name"));
        assert!(text.contains("3840 x 2160"));
    }
}
