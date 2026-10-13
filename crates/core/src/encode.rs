//! Writes pictures to disk. A picture is written under a temporary name and renamed when complete,
//! so an interrupted write never leaves a half-written file behind.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use image::DynamicImage;

use crate::error::CoreError;

/// Writes `img` as a JPEG at `path` with the given quality, through `<name>.tmp`.
pub fn write_jpeg(path: &Path, img: &DynamicImage, quality: u8) -> Result<(), CoreError> {
    let tmp = tmp_path(path);
    if let Err(source) = write_file(&tmp, img, quality) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_error(path, source));
    }
    std::fs::rename(&tmp, path).map_err(|source| {
        let _ = std::fs::remove_file(&tmp);
        io_error(path, source)
    })
}

fn write_file(path: &Path, img: &DynamicImage, quality: u8) -> std::io::Result<()> {
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    img.write_with_encoder(encoder)
        .map_err(std::io::Error::other)?;
    out.flush()
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

fn io_error(path: &Path, source: std::io::Error) -> CoreError {
    CoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn writes_jpeg_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.jpg");
        let img = DynamicImage::ImageRgb8(RgbImage::new(4, 4));
        write_jpeg(&path, &img, 90).unwrap();
        assert!(path.is_file());
        assert!(!tmp_path(&path).exists());
    }

    #[test]
    fn failed_write_returns_the_path_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("out.jpg");
        let img = DynamicImage::ImageRgb8(RgbImage::new(4, 4));
        let err = write_jpeg(&path, &img, 90).unwrap_err();
        assert!(matches!(err, CoreError::Io { path: ref p, .. } if *p == path));
        assert!(!tmp_path(&path).exists());
    }
}
