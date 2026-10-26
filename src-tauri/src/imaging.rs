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
    let img = open(source)?;
    std::fs::create_dir_all(dir)?;
    let thumb = dir.join(format!("{stem}_thumb.jpg"));
    let preview = dir.join(format!("{stem}_preview.jpg"));
    std::fs::write(
        &thumb,
        jpeg_bytes(&fit_long_edge(&img, THUMB_LONG_EDGE), 82)?,
    )?;
    std::fs::write(
        &preview,
        jpeg_bytes(&fit_long_edge(&img, PREVIEW_LONG_EDGE), 90)?,
    )?;
    Ok((
        thumb.to_string_lossy().into_owned(),
        preview.to_string_lossy().into_owned(),
    ))
}

/// The base64 JPEG of an image with its long edge capped.
pub fn encode_for_upload(path: &Path, long_edge: u32, quality: u8) -> Result<String> {
    let img = open(path)?;
    Ok(b64(&jpeg_bytes(&fit_long_edge(&img, long_edge), quality)?))
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
