//! Renders one picture: turns and crops it, then shrinks it to the output size.

use image::imageops::{self, FilterType};
use image::{DynamicImage, GrayImage, Rgb, RgbImage};

use crate::geometry::{Resize, Size, base_size, frame_to_src, output_size};
use crate::model::Adjust;
use crate::tone::{apply, lookup, lut, to_gray};

/// The picture turned and cropped by `adj`, at `out_w` by `out_h` pixels. Pixels are sampled
/// bilinearly; positions outside the source are white.
pub fn rotate_crop(src: &RgbImage, adj: &Adjust, out_w: u32, out_h: u32) -> RgbImage {
    let (w0, h0) = src.dimensions();
    let src_size = Size {
        w: w0 as f32,
        h: h0 as f32,
    };
    let base = base_size(src_size, adj.quarter_turns);
    let crop = adj.crop;
    let (cw, ch) = (crop.w * base.w, crop.h * base.h);
    let (centre_x, centre_y) = (crop.cx * base.w, crop.cy * base.h);
    let (ow, oh) = (out_w as f32, out_h as f32);
    RgbImage::from_fn(out_w, out_h, |u, v| {
        let px = centre_x + ((u as f32 + 0.5) / ow - 0.5) * cw;
        let py = centre_y + ((v as f32 + 0.5) / oh - 0.5) * ch;
        let (sx, sy) = frame_to_src((px, py), src_size, adj.quarter_turns, adj.angle_deg);
        sample(src, sx, sy)
    })
}

/// The picture rendered at its output size: turned, cropped, then shrunk by `resize` with Lanczos.
pub fn render_rgb(src: &RgbImage, adj: &Adjust, resize: Resize) -> RgbImage {
    let (w0, h0) = src.dimensions();
    let base = base_size(
        Size {
            w: w0 as f32,
            h: h0 as f32,
        },
        adj.quarter_turns,
    );
    let (mid_w, mid_h) = output_size(base, adj.crop, Resize::None);
    let mid = rotate_crop(src, adj, mid_w, mid_h);
    let (out_w, out_h) = output_size(base, adj.crop, resize);
    if (out_w, out_h) == (mid_w, mid_h) {
        mid
    } else {
        imageops::resize(&mid, out_w, out_h, FilterType::Lanczos3)
    }
}

/// The picture as it is exported: rendered, then greyscale if set, then the brightness and contrast
/// table unless both are zero. A greyscale picture is returned as one channel.
pub fn render(src: &RgbImage, adj: &Adjust, resize: Resize) -> DynamicImage {
    let mut img = render_rgb(src, adj, resize);
    let table =
        (adj.brightness != 0 || adj.contrast != 0).then(|| lut(adj.brightness, adj.contrast));
    if adj.grayscale {
        let mut gray = to_gray(&img);
        if let Some(table) = &table {
            apply_luma(&mut gray, table);
        }
        return DynamicImage::ImageLuma8(gray);
    }
    if let Some(table) = &table {
        apply(&mut img, table);
    }
    DynamicImage::ImageRgb8(img)
}

fn apply_luma(gray: &mut GrayImage, table: &[u8; 256]) {
    for pixel in gray.pixels_mut() {
        pixel.0[0] = lookup(table, pixel.0[0]);
    }
}

/// Source positions beyond this are far outside any picture; clamping keeps the integer maths safe.
const LIMIT: f32 = 1.0e9;

/// Bilinear sample at a continuous source position, where pixel `i` covers `i..i + 1`.
fn sample(src: &RgbImage, sx: f32, sy: f32) -> Rgb<u8> {
    let (fx, fy) = (
        (sx - 0.5).clamp(-LIMIT, LIMIT),
        (sy - 0.5).clamp(-LIMIT, LIMIT),
    );
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let mut acc = [0.0f32; 3];
    for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            let weight = wx * wy;
            for (a, c) in acc.iter_mut().zip(texel(src, x0 + dx, y0 + dy)) {
                *a += weight * c;
            }
        }
    }
    Rgb(acc.map(|v| v.round().clamp(0.0, 255.0) as u8))
}

/// The colour of a source pixel as floats, or white when the position is outside the source.
fn texel(src: &RgbImage, x: i64, y: i64) -> [f32; 3] {
    let (w, h) = src.dimensions();
    let inside = x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h);
    if !inside {
        return [255.0; 3];
    }
    src.get_pixel_checked(x as u32, y as u32)
        .map_or([255.0; 3], |p| p.0.map(f32::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CropRect;

    use pagecrop_testkit::page_image;

    fn gradient(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| Rgb([(x * 30) as u8, (y * 50) as u8, 7]))
    }

    fn crop_only(cx: f32, cy: f32, w: f32, h: f32) -> Adjust {
        Adjust {
            crop: CropRect { cx, cy, w, h },
            ..Adjust::default()
        }
    }

    #[test]
    fn identity_returns_identical_pixels() {
        let src = gradient(8, 6);
        let out = render_rgb(&src, &Adjust::default(), Resize::None);
        assert_eq!(out, src);
    }

    #[test]
    fn one_turn_equals_imageops_rotate90() {
        let src = gradient(8, 6);
        let adj = Adjust {
            quarter_turns: 1,
            ..Adjust::default()
        };
        let out = render_rgb(&src, &adj, Resize::None);
        assert_eq!(out, image::imageops::rotate90(&src));
    }

    #[test]
    fn right_half_crop_equals_crop_imm() {
        let src = gradient(8, 6);
        let out = render_rgb(&src, &crop_only(0.25, 0.0, 0.5, 1.0), Resize::None);
        let expected = image::imageops::crop_imm(&src, 4, 0, 4, 6).to_image();
        assert_eq!(out, expected);
    }

    #[test]
    fn outside_source_is_white() {
        let src = gradient(4, 4);
        let out = rotate_crop(&src, &crop_only(0.0, 0.0, 2.0, 2.0), 8, 8);
        assert_eq!(out.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(out.get_pixel(4, 4).0[2], 7);
    }

    #[test]
    fn straightened_page_fills_frame() {
        let img = page_image(800, 600, 0, 4.0);
        let crop = CropRect {
            cx: 0.0,
            cy: 0.0,
            w: 0.66,
            h: 0.76,
        };
        let straight = render_rgb(
            &img,
            &Adjust {
                angle_deg: -4.0,
                crop,
                ..Adjust::default()
            },
            Resize::None,
        );
        assert_eq!(straight.dimensions(), (528, 456));
        let (w, h) = straight.dimensions();
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert!(straight.get_pixel(x, y).0[0] > 200, "corner ({x},{y})");
        }
        let skewed = render_rgb(
            &img,
            &Adjust {
                crop,
                ..Adjust::default()
            },
            Resize::None,
        );
        let corners = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)];
        assert!(
            corners
                .iter()
                .any(|&(x, y)| skewed.get_pixel(x, y).0[0] < 100)
        );
    }

    #[test]
    fn long_edge_resize_gives_expected_size() {
        let src = RgbImage::new(1600, 1200);
        let adj = crop_only(0.0, 0.0, 0.66, 0.76);
        let out = render_rgb(&src, &adj, Resize::LongEdge(800));
        assert_eq!(out.dimensions(), (800, 691));
    }

    #[test]
    fn render_gray_returns_luma8() {
        let src = gradient(8, 6);
        let adj = Adjust {
            grayscale: true,
            ..Adjust::default()
        };
        assert!(matches!(
            render(&src, &adj, Resize::None),
            DynamicImage::ImageLuma8(_)
        ));
    }
}
