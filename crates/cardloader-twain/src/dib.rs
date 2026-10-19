//! Conversion of a packed Windows DIB (what `DAT_IMAGENATIVEXFER` hands us on
//! Windows: a `BITMAPINFOHEADER`, optional palette/bitfields, then pixel rows)
//! into 8-bit gray or RGB pixels, plus PNG writing. Pure Rust, cross-platform.
#![cfg_attr(not(windows), allow(dead_code))]

use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Rgb8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Top-down rows, tightly packed (1 or 3 bytes per pixel).
    pub pixels: Vec<u8>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Decode a packed DIB. `data` starts at the `BITMAPINFOHEADER` (no `BITMAPFILEHEADER`).
pub fn decode_dib(data: &[u8]) -> Result<Image, String> {
    if data.len() < 40 {
        return Err(format!("DIB too small ({} bytes)", data.len()));
    }
    let header_size = u32_at(data, 0) as usize;
    if !(40..=124).contains(&header_size) {
        return Err(format!("unsupported DIB header size {header_size}"));
    }
    let width = u32_at(data, 4) as i32;
    let raw_height = u32_at(data, 8) as i32;
    let bpp = u16_at(data, 14);
    let compression = u32_at(data, 16);
    let clr_used = u32_at(data, 32) as usize;
    if width <= 0 || raw_height == 0 {
        return Err(format!("bad DIB dimensions {width}x{raw_height}"));
    }
    let top_down = raw_height < 0;
    let (w, h) = (width as usize, raw_height.unsigned_abs() as usize);

    // Palette (RGBQUAD = B,G,R,reserved) for <= 8 bpp; BI_BITFIELDS masks follow a
    // 40-byte header for 16/32 bpp. We only accept the standard 8-8-8 layout.
    let mut offset = header_size;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    match (bpp, compression) {
        (1 | 4 | 8, BI_RGB) => {
            let n = if clr_used == 0 {
                1usize << bpp
            } else {
                clr_used.min(256)
            };
            let end = offset + n * 4;
            if data.len() < end {
                return Err("DIB palette truncated".into());
            }
            palette = data[offset..end]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|q| [q[2], q[1], q[0]])
                .collect();
            offset = end;
        }
        (24, BI_RGB) | (32, BI_RGB) => {}
        (32, BI_BITFIELDS) => {
            if header_size == 40 {
                offset += 12;
            }
        }
        _ => {
            return Err(format!(
                "unsupported DIB format: {bpp} bpp, compression {compression}"
            ))
        }
    }

    let stride = (w * bpp as usize).div_ceil(32) * 4; // rows are padded to 4 bytes
    let needed = offset + stride * h;
    if data.len() < needed {
        return Err(format!(
            "DIB pixel data truncated ({} < {needed})",
            data.len()
        ));
    }
    let gray_palette = palette.iter().all(|c| c[0] == c[1] && c[1] == c[2]);
    let format = if bpp <= 8 && gray_palette {
        PixelFormat::Gray8
    } else {
        PixelFormat::Rgb8
    };
    let channels = if format == PixelFormat::Gray8 { 1 } else { 3 };
    let mut pixels = Vec::with_capacity(w * h * channels);

    for y in 0..h {
        let src_row = if top_down { y } else { h - 1 - y };
        let row = &data[offset + src_row * stride..offset + src_row * stride + stride];
        for x in 0..w {
            let rgb = match bpp {
                24 => [row[x * 3 + 2], row[x * 3 + 1], row[x * 3]],
                32 => [row[x * 4 + 2], row[x * 4 + 1], row[x * 4]],
                _ => {
                    let bits = bpp as usize;
                    let byte = row[x * bits / 8];
                    let shift = 8 - bits - (x * bits % 8);
                    let idx = (byte >> shift) as usize & ((1 << bits) - 1);
                    *palette.get(idx).unwrap_or(&[0, 0, 0])
                }
            };
            match format {
                PixelFormat::Gray8 => pixels.push(rgb[0]),
                PixelFormat::Rgb8 => pixels.extend_from_slice(&rgb),
            }
        }
    }
    Ok(Image {
        width: w as u32,
        height: h as u32,
        format,
        pixels,
    })
}

/// Write `img` as PNG to `path`, going through `<path>.tmp` + rename so that
/// file watchers never observe a partially written image.
pub fn write_png_atomic(path: &Path, img: &Image, dpi: Option<f64>) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let result = (|| {
        let file = fs::File::create(&tmp)?;
        let mut w = BufWriter::new(file);
        {
            let mut enc = png::Encoder::new(&mut w, img.width, img.height);
            enc.set_color(match img.format {
                PixelFormat::Gray8 => png::ColorType::Grayscale,
                PixelFormat::Rgb8 => png::ColorType::Rgb,
            });
            enc.set_depth(png::BitDepth::Eight);
            enc.set_compression(png::Compression::Fast);
            if let Some(dpi) = dpi.filter(|d| *d > 0.0) {
                let ppm = (dpi / 0.0254).round() as u32;
                enc.set_pixel_dims(Some(png::PixelDimensions {
                    xppu: ppm,
                    yppu: ppm,
                    unit: png::Unit::Meter,
                }));
            }
            let mut writer = enc.write_header().map_err(std::io::Error::other)?;
            writer
                .write_image_data(&img.pixels)
                .map_err(std::io::Error::other)?;
            writer.finish().map_err(std::io::Error::other)?;
        }
        w.flush()?;
        w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(w: i32, h: i32, bpp: u16, clr_used: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&40u32.to_le_bytes());
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&bpp.to_le_bytes());
        v.extend_from_slice(&BI_RGB.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        v.extend_from_slice(&11811i32.to_le_bytes());
        v.extend_from_slice(&11811i32.to_le_bytes());
        v.extend_from_slice(&clr_used.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    }

    #[test]
    fn rgb24_bottom_up_2x2() {
        // Visual image (top-down): red, green / blue, white.
        let mut d = header(2, 2, 24, 0);
        // Bottom row first, BGR, 6 bytes + 2 padding.
        d.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]); // blue, white
        d.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]); // red, green
        let img = decode_dib(&d).unwrap();
        assert_eq!(
            (img.width, img.height, img.format),
            (2, 2, PixelFormat::Rgb8)
        );
        assert_eq!(
            img.pixels,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
        );
    }

    #[test]
    fn rgb24_top_down() {
        let mut d = header(1, -2, 24, 0);
        d.extend_from_slice(&[0, 0, 255, 0]); // red (top)
        d.extend_from_slice(&[255, 0, 0, 0]); // blue
        let img = decode_dib(&d).unwrap();
        assert_eq!(img.pixels, vec![255, 0, 0, 0, 0, 255]);
    }

    #[test]
    fn paletted_8bit_gray_and_color() {
        // Inverted gray palette: index i -> 255 - i.
        let mut d = header(3, 2, 8, 256);
        for i in 0..=255u8 {
            let g = 255 - i;
            d.extend_from_slice(&[g, g, g, 0]);
        }
        d.extend_from_slice(&[10, 20, 30, 0]); // bottom row (+1 pad)
        d.extend_from_slice(&[0, 255, 1, 0]); // top row
        let img = decode_dib(&d).unwrap();
        assert_eq!(img.format, PixelFormat::Gray8);
        assert_eq!(img.pixels, vec![255, 0, 254, 245, 235, 225]);

        // A non-gray palette yields RGB.
        let mut d = header(2, 1, 8, 2);
        d.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]); // red, green
        d.extend_from_slice(&[1, 0, 0, 0]);
        let img = decode_dib(&d).unwrap();
        assert_eq!(img.format, PixelFormat::Rgb8);
        assert_eq!(img.pixels, vec![0, 255, 0, 255, 0, 0]);
    }

    #[test]
    fn one_bit_msb_first() {
        let mut d = header(10, 1, 1, 0);
        d.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]); // 0=black, 1=white
        d.extend_from_slice(&[0b1010_0000, 0b0100_0000, 0, 0]);
        let img = decode_dib(&d).unwrap();
        assert_eq!(img.format, PixelFormat::Gray8);
        assert_eq!(img.pixels, vec![255, 0, 255, 0, 0, 0, 0, 0, 0, 255]);
    }

    #[test]
    fn rejects_truncated() {
        let mut d = header(2, 2, 24, 0);
        d.extend_from_slice(&[0; 8]);
        assert!(decode_dib(&d).is_err());
        assert!(decode_dib(&[0; 10]).is_err());
    }

    #[test]
    fn png_is_written_atomically() {
        let dir =
            std::env::temp_dir().join(format!("cardloader-twain-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t_0001.png");
        let img = Image {
            width: 2,
            height: 1,
            format: PixelFormat::Rgb8,
            pixels: vec![1; 6],
        };
        write_png_atomic(&path, &img, Some(300.0)).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert!(!dir.join("t_0001.png.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
