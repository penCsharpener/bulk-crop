//! Synthetic book pages for tests and manual runs.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use filetime::FileTime;
use image::{Rgb, RgbImage};

const TABLE: [u8; 3] = [40, 40, 40];
const PAGE: [u8; 3] = [245, 245, 240];
const INK: [u8; 3] = [30, 30, 30];
/// Modification time of the first page written with `reverse_mtime`, in seconds since the epoch.
const BOOK_EPOCH: i64 = 1_700_000_000;

/// One page on a dark table, turned clockwise by `angle_deg` around the centre of the picture.
/// Each page has `index + 1` dark squares in a band near its top, so pages can be told apart.
pub fn page_image(w: u32, h: u32, index: u32, angle_deg: f32) -> RgbImage {
    page_image_at(w, h, index, angle_deg, 0.0)
}

/// Like `page_image`, but the page centre is moved right by `shift * w` (left when negative) and the
/// page turns about its own centre.
pub fn page_image_at(w: u32, h: u32, index: u32, angle_deg: f32, shift: f32) -> RgbImage {
    let (wf, hf) = (w as f32, h as f32);
    let (cx, cy) = (wf / 2.0 + shift * wf, hf / 2.0);
    let (page_w, page_h) = (0.70 * wf, 0.80 * hf);
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    RgbImage::from_fn(w, h, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        // Turn the pixel back by -angle to get page-local coordinates.
        let lx = dx * cos + dy * sin;
        let ly = -dx * sin + dy * cos;
        let u = lx / page_w + 0.5;
        let v = ly / page_h + 0.5;
        page_colour(u, v, index)
    })
}

/// The colour at (`u`, `v`) on a page, both 0 to 1 across the page; table colour outside it.
fn page_colour(u: f32, v: f32, index: u32) -> Rgb<u8> {
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return Rgb(TABLE);
    }
    if is_text_line(u, v) || is_marker(u, v, index) {
        Rgb(INK)
    } else {
        Rgb(PAGE)
    }
}

/// One page on a dark table, photographed from below: the page is 0.8 * `h` high and centred, its
/// bottom edge is 0.7 * `w` wide and its top edge 0.7 * `w` * (1 - `squeeze`), the sides straight.
/// Colours, text lines and markers are those of `page_image`.
pub fn keystone_image(w: u32, h: u32, index: u32, squeeze: f32) -> RgbImage {
    let (wf, hf) = (w as f32, h as f32);
    RgbImage::from_fn(w, h, |x, y| {
        let v = (y as f32 + 0.5 - 0.1 * hf) / (0.8 * hf);
        let half = 0.35 * wf * (1.0 - squeeze * (1.0 - v));
        let u = (x as f32 + 0.5 - wf / 2.0) / (2.0 * half) + 0.5;
        page_colour(u, v, index)
    })
}

fn is_text_line(u: f32, v: f32) -> bool {
    (0.1..0.9).contains(&u) && (0.15..0.9).contains(&v) && ((v * 40.0) as u32) % 2 == 1
}

fn is_marker(u: f32, v: f32, index: u32) -> bool {
    (0.04..0.09).contains(&v)
        && (0..=index).any(|k| {
            let start = 0.1 + 0.05 * k as f32;
            (start..start + 0.03).contains(&u)
        })
}

/// Writes `count` pages as `page_001.jpg`, `page_002.jpg`, ... at quality 90 into `dir`,
/// creating `dir` if needed. Page `i` (counted from 0) is tilted by `i * drift_deg`.
/// With `reverse_mtime`, later pages get older modification times, so the last page is the oldest.
pub fn write_book(
    dir: &Path,
    count: u32,
    w: u32,
    h: u32,
    drift_deg: f32,
    reverse_mtime: bool,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).map_err(|e| with_path(e, dir))?;
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let path = dir.join(format!("page_{:03}.jpg", i + 1));
        let img = page_image(w, h, i, i as f32 * drift_deg);
        write_jpeg(&img, &path).map_err(|e| with_path(e, &path))?;
        if reverse_mtime {
            let secs = BOOK_EPOCH - i64::from(i) * 60;
            filetime::set_file_mtime(&path, FileTime::from_unix_time(secs, 0))
                .map_err(|e| with_path(e, &path))?;
        }
        paths.push(path);
    }
    Ok(paths)
}

/// Writes `count` pages like `write_book`, every page from `keystone_image` with the given `squeeze`.
pub fn write_keystone_book(
    dir: &Path,
    count: u32,
    w: u32,
    h: u32,
    squeeze: f32,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).map_err(|e| with_path(e, dir))?;
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let path = dir.join(format!("page_{:03}.jpg", i + 1));
        let img = keystone_image(w, h, i, squeeze);
        write_jpeg(&img, &path).map_err(|e| with_path(e, &path))?;
        paths.push(path);
    }
    Ok(paths)
}

/// How far odd and even photos of a two-sided book sit and tilt: `(angle_deg, shift)` for the photo
/// at `index`, counted from 0. Photos 1, 3, ... sit right, photos 2, 4, ... sit left.
fn two_sided_look(index: u32) -> (f32, f32) {
    if index.is_multiple_of(2) {
        (1.5, 0.08)
    } else {
        (-1.0, -0.08)
    }
}

/// Writes `count` pages like `write_book`, as a book photographed page by page: odd photos show their
/// page further right and turned +1.5 degrees, even photos further left and turned -1.0 degrees.
pub fn write_two_sided_book(
    dir: &Path,
    count: u32,
    w: u32,
    h: u32,
) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).map_err(|e| with_path(e, dir))?;
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let path = dir.join(format!("page_{:03}.jpg", i + 1));
        let (angle, shift) = two_sided_look(i);
        let img = page_image_at(w, h, i, angle, shift);
        write_jpeg(&img, &path).map_err(|e| with_path(e, &path))?;
        paths.push(path);
    }
    Ok(paths)
}

fn write_jpeg(img: &RgbImage, path: &Path) -> std::io::Result<()> {
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
    img.write_with_encoder(encoder)
        .map_err(std::io::Error::other)?;
    out.flush()
}

/// Keeps the error kind and names the path in the message.
fn with_path(e: std::io::Error, path: &Path) -> std::io::Error {
    std::io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_centre_is_page_colour() {
        let img = page_image(800, 600, 0, 0.0);
        assert_eq!(img.get_pixel(400, 300).0, [245, 245, 240]);
    }

    #[test]
    fn corner_is_background() {
        let img = page_image(800, 600, 0, 0.0);
        assert_eq!(img.get_pixel(0, 0).0, [40, 40, 40]);
    }

    #[test]
    fn write_book_creates_count_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = write_book(dir.path(), 3, 80, 60, 0.0, false).unwrap();
        assert_eq!(paths.len(), 3);
        for name in ["page_001.jpg", "page_002.jpg", "page_003.jpg"] {
            assert!(dir.path().join(name).is_file(), "missing {name}");
        }
    }

    #[test]
    fn two_sided_pages_sit_left_and_right() {
        let (w, h) = (800, 600);
        let at = |i| {
            let (angle, shift) = two_sided_look(i);
            page_image_at(w, h, i, angle, shift)
        };
        let (x, y) = ((0.15 * w as f32) as u32, h / 2);
        assert_eq!(at(0).get_pixel(x, y).0, TABLE);
        assert_ne!(at(1).get_pixel(x, y).0, TABLE);
    }

    #[test]
    fn keystone_top_is_narrower() {
        let (w, h) = (800, 600);
        let img = keystone_image(w, h, 0, 0.2);
        let x = (0.5 * w as f32 - 0.33 * w as f32) as u32;
        let top = (0.12 * h as f32) as u32;
        let bottom = (0.88 * h as f32) as u32;
        assert_eq!(img.get_pixel(x, top).0, TABLE);
        assert_eq!(img.get_pixel(x, bottom).0, PAGE);
    }
}
