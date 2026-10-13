//! Brightness, contrast and greyscale. The tone is a table of 256 values, one per input level.

use image::{GrayImage, Luma, RgbImage};

/// The output level for every input level, from brightness and contrast (each -100 to 100).
pub fn lut(brightness: i32, contrast: i32) -> [u8; 256] {
    let k = 1.0 + contrast as f32 / 100.0;
    let shift = brightness as f32 * 1.275;
    let mut table = [0u8; 256];
    for (v, out) in table.iter_mut().enumerate() {
        let x = (v as f32 - 127.5) * k + 127.5 + shift;
        *out = x.round().clamp(0.0, 255.0) as u8;
    }
    table
}

/// Applies the table to every channel of every pixel.
pub fn apply(img: &mut RgbImage, table: &[u8; 256]) {
    for pixel in img.pixels_mut() {
        for channel in pixel.0.iter_mut() {
            *channel = lookup(table, *channel);
        }
    }
}

/// The table entry for `v`. Every u8 is a valid index, so the fallback is never used.
pub(crate) fn lookup(table: &[u8; 256], v: u8) -> u8 {
    table.get(usize::from(v)).copied().unwrap_or(v)
}

/// The greyscale picture, with luma = round(0.299 R + 0.587 G + 0.114 B).
pub fn to_gray(img: &RgbImage) -> GrayImage {
    let mut gray = GrayImage::new(img.width(), img.height());
    for (src, dst) in img.pixels().zip(gray.pixels_mut()) {
        let [r, g, b] = src.0;
        let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
        *dst = Luma([luma.round().clamp(0.0, 255.0) as u8]);
    }
    gray
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn zero_zero_is_identity_table() {
        let table = lut(0, 0);
        for v in 0..=255u8 {
            assert_eq!(table[usize::from(v)], v);
        }
    }

    #[test]
    fn brightness_100_maps_128_to_255() {
        assert_eq!(lut(100, 0)[128], 255);
    }

    #[test]
    fn contrast_minus_100_flattens_to_mid_grey() {
        for v in lut(0, -100) {
            assert!(v == 127 || v == 128, "entry {v}");
        }
    }

    #[test]
    fn table_is_monotonic_for_positive_contrast() {
        let table = lut(0, 50);
        assert!(table.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn gray_of_pure_green_is_150() {
        let img = RgbImage::from_pixel(1, 1, Rgb([0, 255, 0]));
        assert_eq!(to_gray(&img).get_pixel(0, 0).0, [150]);
    }
}
