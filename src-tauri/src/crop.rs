//! Cutting a card out of its picture at four corners someone else found (the local model, an AI agent over MCP):
//! the four-sided shape is mapped onto an upright rectangle, which straightens a tilted or skewed card. Ximilar's
//! crops don't come through here; it cuts its own.

use image::{DynamicImage, Rgb, RgbImage};

use crate::imaging;

pub type Point = [f64; 2];

/// Corners this far outside the picture are still taken (and pulled onto its edge): a card touching the edge is
/// easily read a few pixels over it.
const OUTSIDE: f64 = 0.02;
/// The smallest card worth cutting, as a part of the picture's area.
const MIN_AREA: f64 = 0.01;
/// The pictures compared to tell a turned preview from its scan are shrunk to this many pixels a side.
const TURN_PROBE: u32 = 24;

/// A card's four corners as fractions of its picture (0–1 across and down), clockwise from the corner that
/// becomes the cut's top-left. Fractions, so corners found on a small copy of the picture cut the full scan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad([Point; 4]);

impl Quad {
    /// Corners given in the pixels of a picture `size` wide and high.
    pub fn from_pixels(size: (f64, f64), corners: [Point; 4]) -> Result<Quad, String> {
        let (w, h) = size;
        if !(w.is_finite() && h.is_finite() && w >= 1.0 && h >= 1.0) {
            return Err("the picture's size must be its width and height in pixels".into());
        }
        Quad::new(corners.map(|[x, y]| [x / w, y / h]))
    }

    /// Corners as fractions of the picture: a four-sided shape that doesn't fold, given clockwise (the other way
    /// round would cut the card mirrored).
    pub fn new(corners: [Point; 4]) -> Result<Quad, String> {
        let mut points = corners;
        for p in &mut points {
            for v in p.iter_mut() {
                if !v.is_finite() || *v < -OUTSIDE || *v > 1.0 + OUTSIDE {
                    return Err("a corner is outside the picture".into());
                }
                *v = v.clamp(0.0, 1.0);
            }
        }
        // The turn at each corner: all the same way for a shape that doesn't fold or cross itself.
        let turns: Vec<f64> = (0..4)
            .map(|i| {
                let (a, b, c) = (points[i], points[(i + 1) % 4], points[(i + 2) % 4]);
                (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
            })
            .collect();
        if turns.iter().all(|t| *t < 0.0) {
            return Err(
                "the corners go counter-clockwise: give them clockwise, starting at the card's top-left"
                    .into(),
            );
        }
        if !turns.iter().all(|t| *t > 0.0) {
            return Err(
                "the corners don't make a four-sided shape: give them in order around the card"
                    .into(),
            );
        }
        let area = (0..4)
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % 4]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            .abs()
            / 2.0;
        if area < MIN_AREA {
            return Err("the corners enclose too little of the picture to be a card".into());
        }
        Ok(Quad(points))
    }

    /// Four points in any order, as a card lying the way the picture shows it: clockwise from the corner nearest
    /// the picture's top-left. For a finder whose naming of the corners isn't to be trusted (a small model).
    pub fn in_picture_order(mut points: [Point; 4]) -> Result<Quad, String> {
        let n = points.len() as f64;
        let cx = points.iter().map(|p| p[0]).sum::<f64>() / n;
        let cy = points.iter().map(|p| p[1]).sum::<f64>() / n;
        // With y pointing down, a growing angle goes clockwise on the screen.
        points.sort_by(|a, b| {
            let angle = |p: &Point| (p[1] - cy).atan2(p[0] - cx);
            angle(a).total_cmp(&angle(b))
        });
        let first = (0..4)
            .min_by(|a, b| {
                let far = |i: &usize| points[*i][0] + points[*i][1];
                far(a).total_cmp(&far(b))
            })
            .unwrap_or(0);
        points.rotate_left(first);
        Quad::new(points)
    }

    /// The same corners on the picture turned upside down.
    pub fn turned(self) -> Quad {
        Quad(self.0.map(|[x, y]| [1.0 - x, 1.0 - y]))
    }

    /// The corners, for the card's stored record (four decimals: a tenth of a pixel on a 1000 px picture).
    pub fn points(&self) -> [Point; 4] {
        self.0.map(|p| p.map(|v| (v * 10_000.0).round() / 10_000.0))
    }
}

/// Whether `scan` is `preview`'s picture upside down. Turning a card upright turns its preview, but a file in the
/// user's own folder is never changed, and nothing records which was turned: the pictures themselves tell. One
/// that looks the same both ways gives the same cut either way.
pub fn turned(scan: &DynamicImage, preview: &DynamicImage) -> bool {
    let probe = |img: &DynamicImage| img.thumbnail_exact(TURN_PROBE, TURN_PROBE).to_luma8();
    let (scan, preview) = (probe(scan), probe(preview));
    let off = |turn: bool| -> u64 {
        let n = scan.as_raw().len();
        (0..n)
            .map(|i| {
                let s = scan.as_raw()[if turn { n - 1 - i } else { i }];
                u64::from(s.abs_diff(preview.as_raw()[i]))
            })
            .sum()
    };
    off(true) < off(false)
}

/// The card inside `quad` as an upright picture: as wide and high as the shape's sides are long in `img`, never
/// over the size grading scans are capped at.
pub fn cut(img: &DynamicImage, quad: &Quad) -> DynamicImage {
    let src = img.to_rgb8();
    let (sw, sh) = (f64::from(src.width()), f64::from(src.height()));
    let p = quad.0.map(|[x, y]| [x * sw, y * sh]);
    let side = |a: Point, b: Point| (a[0] - b[0]).hypot(a[1] - b[1]);
    let width = (side(p[0], p[1]) + side(p[3], p[2])) / 2.0;
    let height = (side(p[1], p[2]) + side(p[0], p[3])) / 2.0;
    let shrink = (f64::from(imaging::GRADING_LONG_EDGE) / width.max(height)).min(1.0);
    let w = ((width * shrink).round() as u32).max(1);
    let h = ((height * shrink).round() as u32).max(1);

    // The unit square onto the four corners (Heckbert, "Fundamentals of Texture Mapping and Image Warping", 1989):
    // x = (a·u + b·v + c) / (g·u + k·v + 1), y = (d·u + e·v + f) / (g·u + k·v + 1).
    let [[x0, y0], [x1, y1], [x2, y2], [x3, y3]] = p;
    let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
    let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
    let den = dx1 * dy2 - dy1 * dx2;
    let g = (dx3 * dy2 - dy3 * dx2) / den;
    let k = (dx1 * dy3 - dy1 * dx3) / den;
    let (a, b, c) = (x1 - x0 + g * x1, x3 - x0 + k * x3, x0);
    let (d, e, f) = (y1 - y0 + g * y1, y3 - y0 + k * y3, y0);

    let mut out = RgbImage::new(w, h);
    for (i, j, pixel) in out.enumerate_pixels_mut() {
        // Pixel centres: the cut's corner pixels sit just inside the shape's corners.
        let u = (f64::from(i) + 0.5) / f64::from(w);
        let v = (f64::from(j) + 0.5) / f64::from(h);
        let z = g * u + k * v + 1.0;
        let x = (a * u + b * v + c) / z - 0.5;
        let y = (d * u + e * v + f) / z - 0.5;
        *pixel = sample(&src, x, y);
    }
    DynamicImage::ImageRgb8(out)
}

/// The picture's colour at a point between its pixels (bilinear), the edge's beyond it.
fn sample(img: &RgbImage, x: f64, y: f64) -> Rgb<u8> {
    let (max_x, max_y) = (f64::from(img.width() - 1), f64::from(img.height() - 1));
    let (x, y) = (x.clamp(0.0, max_x), y.clamp(0.0, max_y));
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as u32, y0 as u32);
    let (x1, y1) = (
        (x0 + 1).min(img.width() - 1),
        (y0 + 1).min(img.height() - 1),
    );
    let (p00, p10) = (img.get_pixel(x0, y0), img.get_pixel(x1, y0));
    let (p01, p11) = (img.get_pixel(x0, y1), img.get_pixel(x1, y1));
    let mut out = [0u8; 3];
    for (n, value) in out.iter_mut().enumerate() {
        let top = f64::from(p00[n]) * (1.0 - fx) + f64::from(p10[n]) * fx;
        let bottom = f64::from(p01[n]) * (1.0 - fx) + f64::from(p11[n]) * fx;
        *value = (top * (1.0 - fy) + bottom * fy).round() as u8;
    }
    Rgb(out)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const RED: [u8; 3] = [220, 30, 30];
    const GREEN: [u8; 3] = [30, 200, 30];
    const BLUE: [u8; 3] = [30, 30, 220];
    const WHITE: [u8; 3] = [240, 240, 240];
    const TABLE: [u8; 3] = [20, 20, 20];

    /// A 100×140 card (its quarters red, green, blue and white from the top-left, row by row) lying on a dark
    /// table, turned by `degrees` around the middle of a `w`×`h` picture: the picture and the card's corners in
    /// its pixels, from the card's own top-left, clockwise.
    pub fn card_on_table(w: u32, h: u32, degrees: f64) -> (DynamicImage, [Point; 4]) {
        let (cw, ch) = (100.0, 140.0);
        let (sin, cos) = degrees.to_radians().sin_cos();
        let (mx, my) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        let mut img = RgbImage::from_pixel(w, h, Rgb(TABLE));
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            // The picture's pixel in the card's own frame.
            let (dx, dy) = (f64::from(x) + 0.5 - mx, f64::from(y) + 0.5 - my);
            let (u, v) = (
                dx * cos + dy * sin + cw / 2.0,
                -dx * sin + dy * cos + ch / 2.0,
            );
            if (0.0..cw).contains(&u) && (0.0..ch).contains(&v) {
                *pixel = Rgb(match (u < cw / 2.0, v < ch / 2.0) {
                    (true, true) => RED,
                    (false, true) => GREEN,
                    (true, false) => BLUE,
                    (false, false) => WHITE,
                });
            }
        }
        let corner = |u: f64, v: f64| {
            let (u, v) = (u - cw / 2.0, v - ch / 2.0);
            [mx + u * cos - v * sin, my + u * sin + v * cos]
        };
        let corners = [
            corner(0.0, 0.0),
            corner(cw, 0.0),
            corner(cw, ch),
            corner(0.0, ch),
        ];
        (DynamicImage::ImageRgb8(img), corners)
    }

    /// The colours in the middle of a cut's quarters, from the top-left, row by row.
    pub fn quarters(cut: &DynamicImage) -> [[u8; 3]; 4] {
        let img = cut.to_rgb8();
        let (w, h) = img.dimensions();
        [(1, 1), (3, 1), (1, 3), (3, 3)].map(|(x, y)| img.get_pixel(w * x / 4, h * y / 4).0)
    }

    fn close(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| a.abs_diff(b) < 12)
    }

    #[test]
    fn cuts_a_tilted_card_upright() {
        for degrees in [0.0, 25.0, -40.0, 90.0, 180.0] {
            let (img, corners) = card_on_table(400, 300, degrees);
            let quad = Quad::from_pixels((400.0, 300.0), corners).unwrap();
            let card = cut(&img, &quad);
            assert!(
                card.width().abs_diff(100) <= 1 && card.height().abs_diff(140) <= 1,
                "{degrees}°: {}×{}",
                card.width(),
                card.height()
            );
            // From the card's own top-left: upright however it lies.
            for (got, want) in quarters(&card).into_iter().zip([RED, GREEN, BLUE, WHITE]) {
                assert!(close(got, want), "{degrees}°: {got:?} for {want:?}");
            }
            // Nothing of the table at the cut's edges.
            let rgb = card.to_rgb8();
            assert!(close(rgb.get_pixel(1, 1).0, RED), "{degrees}°");
            let (w, h) = rgb.dimensions();
            assert!(close(rgb.get_pixel(w - 2, h - 2).0, WHITE), "{degrees}°");
        }
    }

    #[test]
    fn cuts_a_skewed_card() {
        // A card photographed from below: its top edge shorter than its bottom one.
        let (img, _) = card_on_table(400, 300, 0.0);
        let corners = [[160.0, 80.0], [240.0, 80.0], [250.0, 220.0], [150.0, 220.0]];
        let card = cut(&img, &Quad::from_pixels((400.0, 300.0), corners).unwrap());
        assert_eq!((card.width(), card.height()), (90, 140));
        for (got, want) in quarters(&card).into_iter().zip([RED, GREEN, BLUE, WHITE]) {
            assert!(close(got, want), "{got:?} for {want:?}");
        }
    }

    #[test]
    fn a_large_cut_is_capped() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(6000, 500, Rgb(RED)));
        let whole = Quad::new([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
        let card = cut(&img, &whole);
        assert_eq!(card.width(), imaging::GRADING_LONG_EDGE);
        assert_eq!(card.height(), 333);
    }

    #[test]
    fn refuses_bad_quads() {
        let ok = [[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]];
        assert!(Quad::new(ok).is_ok());
        let mirrored = [ok[1], ok[0], ok[3], ok[2]];
        assert!(Quad::new(mirrored)
            .unwrap_err()
            .contains("counter-clockwise"));
        let crossed = [ok[0], ok[2], ok[1], ok[3]];
        assert!(Quad::new(crossed).unwrap_err().contains("four-sided"));
        let dented = [[0.1, 0.1], [0.9, 0.1], [0.3, 0.3], [0.1, 0.9]];
        assert!(Quad::new(dented).unwrap_err().contains("four-sided"));
        let tiny = [[0.5, 0.5], [0.55, 0.5], [0.55, 0.55], [0.5, 0.55]];
        assert!(Quad::new(tiny).unwrap_err().contains("too little"));
        let outside = [[-0.2, 0.1], ok[1], ok[2], ok[3]];
        assert!(Quad::new(outside).unwrap_err().contains("outside"));
        assert!(Quad::new([[f64::NAN, 0.1], ok[1], ok[2], ok[3]]).is_err());
        // A little over the edge is the edge.
        let over = Quad::new([[-0.01, 0.0], [1.01, 0.0], [1.0, 1.02], [0.0, 1.0]]).unwrap();
        assert_eq!(
            over.points(),
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        );

        assert!(Quad::from_pixels((0.0, 100.0), ok).is_err());
        let px = Quad::from_pixels(
            (1000.0, 500.0),
            [[100.0, 50.0], [900.0, 50.0], [900.0, 450.0], [100.0, 450.0]],
        );
        assert_eq!(px.unwrap().points(), ok);
    }

    #[test]
    fn orders_points_as_the_picture() {
        let want = [[0.2, 0.1], [0.8, 0.15], [0.85, 0.9], [0.15, 0.85]];
        // Named wrongly, the other way round, starting anywhere: the same card.
        for order in [[0, 1, 2, 3], [2, 3, 0, 1], [3, 2, 1, 0], [1, 3, 0, 2]] {
            let quad = Quad::in_picture_order(order.map(|i| want[i])).unwrap();
            assert_eq!(quad.points(), want, "{order:?}");
        }
        // Three points in a line aren't a card.
        let line = [[0.1, 0.1], [0.5, 0.1], [0.9, 0.1], [0.5, 0.9]];
        assert!(Quad::in_picture_order(line).is_err());
    }

    #[test]
    fn finds_a_turned_scan() {
        let (scan, corners) = card_on_table(400, 300, 15.0);
        let preview = scan.thumbnail(200, 150);
        assert!(!turned(&scan, &preview));
        assert!(turned(&scan.rotate180(), &preview));
        // The corners found on the preview, turned with it, cut the scan to the picture the preview shows.
        let quad = Quad::from_pixels((400.0, 300.0), corners).unwrap();
        let card = cut(&scan.rotate180(), &quad.turned());
        for (got, want) in quarters(&card).into_iter().zip([RED, GREEN, BLUE, WHITE]) {
            assert!(close(got, want), "{got:?} for {want:?}");
        }
        // One colour all over: neither way is better, so it counts as not turned.
        let plain = DynamicImage::ImageRgb8(RgbImage::from_pixel(50, 70, Rgb(RED)));
        assert!(!turned(&plain, &plain));
    }
}
