//! Decoding, orientation, resizing and encoding of card images.

use std::io::Cursor;
use std::path::Path;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader};

use crate::error::Result;
use crate::model::ScanRotation;

pub const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp"];
/// Long edge of the image sent to recognition and shown in the detail panel.
pub const PREVIEW_LONG_EDGE: u32 = 1600;
pub const THUMB_LONG_EDGE: u32 = 240;
/// Grading wants detail: send the scan as is, but never more than this.
pub const GRADING_LONG_EDGE: u32 = 4000;
/// Cards Cloud keeps at most 1024 px.
pub const CARDS_CLOUD_LONG_EDGE: u32 = 1024;

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Decodes an image and applies its EXIF orientation.
pub fn open(path: &Path) -> Result<DynamicImage> {
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder)?;
    if let Some(orientation) = orientation {
        img.apply_orientation(orientation);
    }
    Ok(img)
}

pub fn fit_long_edge(img: &DynamicImage, long_edge: u32) -> DynamicImage {
    if img.width().max(img.height()) <= long_edge {
        return img.clone();
    }
    img.resize(long_edge, long_edge, FilterType::Lanczos3)
}

pub fn jpeg_bytes(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let rgb = img.to_rgb8();
    JpegEncoder::new_with_quality(Cursor::new(&mut out), quality).encode_image(&rgb)?;
    Ok(out)
}

pub fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Turns a scanned page in place, before anything else reads it.
pub fn rotate_file(path: &Path, rotation: ScanRotation) -> Result<()> {
    match rotation {
        ScanRotation::None => Ok(()),
        ScanRotation::UpsideDown => Ok(open(path)?.rotate180().save(path)?),
    }
}

/// Writes the thumbnail and preview JPEGs of `source` into `dir` as `<stem>_thumb.jpg` / `<stem>_preview.jpg`.
pub fn make_derivatives(source: &Path, dir: &Path, stem: &str) -> Result<(String, String)> {
    write_derivatives(&open(source)?, dir, stem)
}

/// The thumbnail and preview made again from `preview` turned upside down. New names, so nothing shows a
/// cached copy of the old ones.
pub fn turned_derivatives(preview: &Path, dir: &Path, stem: &str) -> Result<(String, String)> {
    write_derivatives(&open(preview)?.rotate180(), dir, stem)
}

/// `preview` turned upside down, saved as `<stem>_back_preview.jpg` in `dir`: a back has no thumbnail.
pub fn turned_preview(preview: &Path, dir: &Path, stem: &str) -> Result<String> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{stem}_back_preview.jpg"));
    let img = fit_long_edge(&open(preview)?.rotate180(), PREVIEW_LONG_EDGE);
    std::fs::write(&path, jpeg_bytes(&img, 90)?)?;
    Ok(path.to_string_lossy().into_owned())
}

fn write_derivatives(img: &DynamicImage, dir: &Path, stem: &str) -> Result<(String, String)> {
    std::fs::create_dir_all(dir)?;
    let thumb = dir.join(format!("{stem}_thumb.jpg"));
    let preview = dir.join(format!("{stem}_preview.jpg"));
    let preview_img = fit_long_edge(img, PREVIEW_LONG_EDGE);
    // From the preview: shrinking a full scan to a thumbnail costs as much as making the preview again.
    std::fs::write(
        &thumb,
        jpeg_bytes(&fit_long_edge(&preview_img, THUMB_LONG_EDGE), 82)?,
    )?;
    std::fs::write(&preview, jpeg_bytes(&preview_img, 90)?)?;
    Ok((
        thumb.to_string_lossy().into_owned(),
        preview.to_string_lossy().into_owned(),
    ))
}

/// A card picture for a marketplace: a JPEG at the preview size.
pub fn listing_jpeg(path: &Path) -> Result<Vec<u8>> {
    let img = open(path)?;
    jpeg_bytes(&fit_long_edge(&img, PREVIEW_LONG_EDGE), 90)
}

/// The base64 JPEG of an image with its long edge capped.
pub fn encode_for_upload(path: &Path, long_edge: u32, quality: u8) -> Result<String> {
    Ok(encode_sized(path, long_edge, quality)?.0)
}

/// [`encode_for_upload`] and the width and height of the picture it made: what coordinates found on it are in.
pub fn encode_sized(path: &Path, long_edge: u32, quality: u8) -> Result<(String, (u32, u32))> {
    let img = fit_long_edge(&open(path)?, long_edge);
    Ok((
        b64(&jpeg_bytes(&img, quality)?),
        (img.width(), img.height()),
    ))
}

/// The JPEG qualities a picture that must fit a size is tried at, best first. Below the last, print turns to
/// blocks: a smaller picture reads better than a coarser one.
const FITTING_QUALITIES: [u8; 3] = [85, 75, 65];
/// A picture made to fit a size is never shrunk below this long edge (it is then sent over the size).
const FITTING_MIN_EDGE: u32 = 512;

/// [`encode_sized`] in at most `max_base64` characters: at a lower quality first, then smaller.
pub fn encode_within(
    path: &Path,
    long_edge: u32,
    max_base64: usize,
) -> Result<(String, (u32, u32))> {
    within(fit_long_edge(&open(path)?, long_edge), max_base64)
}

/// A picture that came as bytes (a listing's, fetched from eBay) as a base64 JPEG with its long edge capped.
pub fn encode_bytes(bytes: &[u8], long_edge: u32, quality: u8) -> Result<(String, (u32, u32))> {
    let img = fit_long_edge(&image::load_from_memory(bytes)?, long_edge);
    Ok((
        b64(&jpeg_bytes(&img, quality)?),
        (img.width(), img.height()),
    ))
}

/// [`encode_bytes`] in at most `max_base64` characters, as [`encode_within`] fits a file.
pub fn encode_bytes_within(
    bytes: &[u8],
    long_edge: u32,
    max_base64: usize,
) -> Result<(String, (u32, u32))> {
    within(
        fit_long_edge(&image::load_from_memory(bytes)?, long_edge),
        max_base64,
    )
}

fn within(mut img: DynamicImage, max_base64: usize) -> Result<(String, (u32, u32))> {
    // Base64 writes three bytes as four characters.
    let max_bytes = max_base64 / 4 * 3;
    loop {
        let mut jpeg = Vec::new();
        for quality in FITTING_QUALITIES {
            jpeg = jpeg_bytes(&img, quality)?;
            if jpeg.len() <= max_bytes {
                break;
            }
        }
        let edge = img.width().max(img.height());
        if jpeg.len() <= max_bytes || edge <= FITTING_MIN_EDGE {
            return Ok((b64(&jpeg), (img.width(), img.height())));
        }
        img = fit_long_edge(&img, (edge * 85 / 100).max(FITTING_MIN_EDGE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn derivatives_are_resized_jpegs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("card.png");
        RgbImage::from_pixel(1000, 2000, Rgb([10, 20, 30]))
            .save(&src)
            .unwrap();
        let (thumb, preview) = make_derivatives(&src, dir.path(), "1").unwrap();
        let thumb = image::open(thumb).unwrap();
        let preview = image::open(preview).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (120, 240));
        assert_eq!((preview.width(), preview.height()), (800, 1600));
    }

    /// A picture that must fit a size keeps its size while a lower quality does it, and is shrunk only then.
    #[test]
    fn fits_a_picture_into_a_size() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("noise.png");
        // Noise: what JPEG compresses worst.
        let mut seed = 7u32;
        RgbImage::from_fn(900, 1260, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
        })
        .save(&src)
        .unwrap();
        let (full, size) = encode_sized(&src, 1024, 85).unwrap();
        assert_eq!(size, (731, 1024));
        // Room enough: the same picture.
        let (same, same_size) = encode_within(&src, 1024, full.len()).unwrap();
        assert_eq!((same.len(), same_size), (full.len(), size));
        // A little less: coarser, as large.
        let (coarser, coarser_size) = encode_within(&src, 1024, full.len() * 9 / 10).unwrap();
        assert!(coarser.len() <= full.len() * 9 / 10);
        assert_eq!(coarser_size, size);
        // Much less: smaller too, and a readable JPEG.
        let (small, (w, h)) = encode_within(&src, 1024, full.len() / 4).unwrap();
        assert!(
            small.len() <= full.len() / 4,
            "{} of {}",
            small.len(),
            full.len()
        );
        assert!((FITTING_MIN_EDGE..1024).contains(&h), "{w}×{h}");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&small)
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (w, h));
        // Never below the smallest size, whatever is asked.
        let (_, (_, h)) = encode_within(&src, 1024, 1000).unwrap();
        assert_eq!(h, FITTING_MIN_EDGE);
    }

    #[test]
    fn rotates_scans_upside_down() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("scan.png");
        let mut img = RgbImage::from_pixel(2, 3, Rgb([0, 0, 0]));
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.save(&src).unwrap();
        rotate_file(&src, ScanRotation::None).unwrap();
        assert_eq!(image::open(&src).unwrap().to_rgb8().get_pixel(0, 0)[0], 255);
        rotate_file(&src, ScanRotation::UpsideDown).unwrap();
        let turned = image::open(&src).unwrap().to_rgb8();
        assert_eq!(turned.dimensions(), (2, 3));
        assert_eq!(turned.get_pixel(1, 2)[0], 255);
        assert_eq!(turned.get_pixel(0, 0)[0], 0);
    }

    #[test]
    fn supported_extensions_ignore_case() {
        assert!(is_supported(Path::new("/x/A.JPG")));
        assert!(is_supported(Path::new("scan_0001.tiff")));
        assert!(!is_supported(Path::new("notes.txt")));
    }
}
