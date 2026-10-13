//! Reads picture files, applies the EXIF orientation and shrinks them.

use std::io::BufReader;
use std::path::Path;

use image::RgbImage;
use image::imageops::{self, flip_horizontal, flip_vertical, rotate90, rotate180, rotate270};

use crate::error::CoreError;

/// The EXIF orientation tag (1 to 8) of a file. Any problem reading it gives 1, meaning "as stored".
pub fn read_orientation(path: &Path) -> u8 {
    let Ok(file) = std::fs::File::open(path) else {
        return 1;
    };
    let Ok(exif) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else {
        return 1;
    };
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .and_then(|value| u8::try_from(value).ok())
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1)
}

/// Turns the picture the way its EXIF orientation says, so that it is upright.
pub fn apply_orientation(img: &RgbImage, orientation: u8) -> RgbImage {
    match orientation {
        2 => flip_horizontal(img),
        3 => rotate180(img),
        4 => flip_vertical(img),
        5 => flip_horizontal(&rotate90(img)),
        6 => rotate90(img),
        7 => flip_horizontal(&rotate270(img)),
        8 => rotate270(img),
        _ => img.clone(),
    }
}

/// Decodes a file to RGB and applies its EXIF orientation.
pub fn load_oriented(path: &Path) -> Result<RgbImage, CoreError> {
    let img = image::ImageReader::open(path)
        .map_err(|source| CoreError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .decode()
        .map_err(|err| CoreError::Decode {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?
        .to_rgb8();
    Ok(apply_orientation(&img, read_orientation(path)))
}

/// Shrinks the picture so its longer side is at most `max`, keeping the aspect ratio.
/// A picture that is already small enough is returned unchanged.
pub fn shrink_to_long_edge(img: &RgbImage, max: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let long = w.max(h);
    if long <= max || w == 0 || h == 0 {
        return img.clone();
    }
    let scale = max as f64 / long as f64;
    let new_w = (w as f64 * scale).round().max(1.0) as u32;
    let new_h = (h as f64 * scale).round().max(1.0) as u32;
    imageops::thumbnail(img, new_w, new_h)
}

/// Parses EXIF date text such as `2026:01:06 16:53:14` into seconds since the epoch, in UTC.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp())
}

/// The date a picture was taken: EXIF `DateTimeOriginal`, else the file's modification time.
pub fn read_taken(path: &Path) -> Option<i64> {
    exif_taken(path).or_else(|| modified_secs(path))
}

fn exif_taken(path: &Path) -> Option<i64> {
    let file = std::fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let field = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)?;
    match &field.value {
        exif::Value::Ascii(parts) => {
            let text = std::str::from_utf8(parts.first()?).ok()?;
            parse_exif_datetime(text.trim_end_matches('\0').trim())
        }
        _ => None,
    }
}

fn modified_secs(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    i64::try_from(secs).ok()
}

/// A date in seconds since the epoch as text for display, or `-` when there is none.
pub fn format_taken(taken: Option<i64>) -> String {
    taken
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map_or_else(
            || "-".to_owned(),
            |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    /// A 3x2 picture whose six pixels all have different colours:
    /// A B C
    /// D E F
    fn six_colours() -> RgbImage {
        let colours = [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 255, 0],
            [0, 255, 255],
            [255, 0, 255],
        ];
        RgbImage::from_fn(3, 2, |x, y| Rgb(colours[(y * 3 + x) as usize]))
    }

    #[test]
    fn orientation_1_to_8_moves_pixels_correctly() {
        let a = [255, 0, 0];
        let c = [0, 0, 255];
        let d = [255, 255, 0];
        let f = [255, 0, 255];
        // Expected colour at (0,0) for orientations 1 to 8.
        let expected = [a, c, f, d, a, d, f, c];
        for (o, colour) in (1u8..=8).zip(expected) {
            let out = apply_orientation(&six_colours(), o);
            let (w, h) = out.dimensions();
            if o <= 4 {
                assert_eq!((w, h), (3, 2), "orientation {o}");
            } else {
                assert_eq!((w, h), (2, 3), "orientation {o}");
            }
            assert_eq!(out.get_pixel(0, 0).0, colour, "orientation {o}");
        }
    }

    #[test]
    fn shrink_1600x1200_to_160_gives_160x120() {
        let img = RgbImage::new(1600, 1200);
        assert_eq!(shrink_to_long_edge(&img, 160).dimensions(), (160, 120));
    }

    #[test]
    fn shrink_never_enlarges() {
        let img = RgbImage::new(100, 80);
        assert_eq!(shrink_to_long_edge(&img, 500).dimensions(), (100, 80));
    }

    #[test]
    fn parses_exif_datetime() {
        assert_eq!(parse_exif_datetime("2026:01:06 16:53:14"), Some(1767718394));
    }

    #[test]
    fn rejects_garbage_datetime() {
        assert_eq!(parse_exif_datetime("not a date"), None);
        assert_eq!(parse_exif_datetime("2026:13:45 00:00:00"), None);
    }

    #[test]
    fn empty_file_is_decode_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jpg");
        std::fs::write(&path, b"").unwrap();
        let err = load_oriented(&path).unwrap_err();
        assert!(matches!(err, CoreError::Decode { .. }));
        assert!(err.to_string().contains("empty.jpg"));
    }
}
