//! Display derivatives of the master art: responsive widths and the social-card crop.
//!
//! > *"Neither do men light a candle, and put it under a bushel, but on a candlestick;
//! > and it giveth light unto all that are in the house."* - Matthew 5:15, KJV
//!
//! WHY re-encode at all: a 960px Commons thumbnail of a Doré engraving is ~490 KB,
//! because engraved line work is the hardest thing JPEG compresses. Resizing with
//! Lanczos3 and encoding at quality 72 keeps the line work and cuts the bytes, so the
//! art arrives fast on a phone. Pixels are only scaled and, for the social card,
//! cropped - arrangement of master art, never alteration of it.

use crate::art::Variant;
use crate::{Error, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader};
use std::path::Path;

/// Display widths, narrowest first.
///
/// 1440 and 1920 serve the 60rem plate column on high-density screens, so the
/// engraved line work stays crisp at 2x; browsers pick the narrowest that fills the
/// slot, so phones never download them.
pub const WIDTHS: [u32; 5] = [480, 720, 960, 1440, 1920];
/// JPEG quality for derivatives.
pub const QUALITY: u8 = 72;
/// JPEG quality for the social card (shown small in feeds, so fewer bytes).
pub const OG_QUALITY: u8 = 62;
/// Social card size (Open Graph / Twitter large image).
pub const OG: (u32, u32) = (1200, 630);

fn img_err(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::Image {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

/// Opens a master and returns it with its pixel size.
///
/// # Errors
/// Fails when the file cannot be read or decoded.
pub fn open(path: &Path) -> Result<DynamicImage> {
    ImageReader::open(path)
        .map_err(|e| img_err(path, e))?
        .with_guessed_format()
        .map_err(|e| img_err(path, e))?
        .decode()
        .map_err(|e| img_err(path, e))
}

/// Drops alpha and keeps greyscale masters greyscale (one channel is a third of the bytes
/// and changes no pixel).
fn flatten(img: DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageLuma8(_) => img,
        DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_) => DynamicImage::ImageLuma8(img.to_luma8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    }
}

fn scaled_height(w0: u32, h0: u32, w: u32) -> u32 {
    let h = u64::from(h0) * u64::from(w) / u64::from(w0.max(1));
    u32::try_from(h.max(1)).unwrap_or(u32::MAX)
}

fn save_jpeg(img: &DynamicImage, path: &Path, quality: u8) -> Result<()> {
    let file = std::fs::File::create(path).map_err(|e| Error::io("create", path, e))?;
    let mut w = std::io::BufWriter::new(file);
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, quality);
    img.write_with_encoder(enc).map_err(|e| img_err(path, e))
}

fn widths_for(w0: u32) -> Vec<u32> {
    let mut widths: Vec<u32> = WIDTHS.iter().copied().filter(|w| *w < w0).collect();
    if widths.len() < WIDTHS.len() {
        widths.push(w0.min(WIDTHS.iter().copied().max().unwrap_or(w0)));
    }
    widths.dedup();
    widths
}

/// Reuses derivatives that are newer than their master, reading only their headers,
/// so a re-fetch after a plan change does not re-encode every image.
fn already_derived(
    master: &Path,
    out_dir: &Path,
    slug: &str,
) -> Option<(Vec<Variant>, Option<Variant>)> {
    let master_time = std::fs::metadata(master).and_then(|m| m.modified()).ok()?;
    let (w0, _) = image::image_dimensions(master).ok()?;
    let mut files: Vec<String> = widths_for(w0)
        .iter()
        .map(|w| format!("{slug}-{w}.jpg"))
        .collect();
    files.push(format!("{slug}-og.jpg"));
    let mut out = Vec::new();
    for file in files {
        let path = out_dir.join(&file);
        let fresh = std::fs::metadata(&path).and_then(|m| m.modified()).ok()? >= master_time;
        if !fresh {
            return None;
        }
        let (width, height) = image::image_dimensions(&path).ok()?;
        out.push(Variant {
            width,
            height,
            file,
        });
    }
    let og = out.pop();
    Some((out, og))
}

/// Writes `<slug>-<width>.jpg` for each display width the master can supply, and the
/// `<slug>-og.jpg` social card.
///
/// # Errors
/// Fails when the master cannot be decoded or a derivative cannot be written.
pub fn derive(
    master: &Path,
    out_dir: &Path,
    slug: &str,
) -> Result<(Vec<Variant>, Option<Variant>)> {
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io("create dir", out_dir, e))?;
    if let Some(done) = already_derived(master, out_dir, slug) {
        return Ok(done);
    }
    let img = flatten(open(master)?);
    let (w0, h0) = img.dimensions();
    let mut variants = Vec::new();
    for w in widths_for(w0) {
        let h = scaled_height(w0, h0, w);
        let resized = if w == w0 {
            img.clone()
        } else {
            img.resize_exact(w, h, FilterType::Lanczos3)
        };
        let file = format!("{slug}-{w}.jpg");
        save_jpeg(&resized, &out_dir.join(&file), QUALITY)?;
        variants.push(Variant {
            width: w,
            height: h,
            file,
        });
    }
    // Social card: full width, a 1.905:1 band whose top sits 30% down the remaining
    // height, which keeps the sky and figures of a portrait plate in frame.
    let card_w = w0.min(OG.0);
    let scaled = if card_w == w0 {
        img
    } else {
        img.resize_exact(card_w, scaled_height(w0, h0, card_w), FilterType::Lanczos3)
    };
    let (sw, sh) = scaled.dimensions();
    let card_h = scaled_height(OG.0, OG.1, sw).min(sh);
    let spare = sh.saturating_sub(card_h);
    let top = u32::try_from(u64::from(spare) * 30 / 100).unwrap_or(0);
    let card = scaled.crop_imm(0, top, sw, card_h);
    let og_file = format!("{slug}-og.jpg");
    save_jpeg(&card, &out_dir.join(&og_file), OG_QUALITY)?;
    let og = Variant {
        width: sw,
        height: card_h,
        file: og_file,
    };
    Ok((variants, Some(og)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_scale_proportionally() {
        assert_eq!(scaled_height(2317, 2920, 960), 1209);
        assert_eq!(scaled_height(1200, 630, 1200), 630);
    }
}
