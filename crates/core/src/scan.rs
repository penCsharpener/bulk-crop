//! Finds the picture files in a folder.

use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// The JPEG files directly inside `dir`, sorted by file name in natural order.
/// Hidden files, `.tmp` files and sub-folders are left out.
pub fn list_jpegs(dir: &Path) -> Result<Vec<PathBuf>, CoreError> {
    let io_err = |source: std::io::Error| CoreError::Io {
        path: dir.to_path_buf(),
        source,
    };
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if is_jpeg_name(&name) && path.is_file() {
            found.push((name, path));
        }
    }
    found.sort_by(|a, b| natord::compare_ignore_case(&a.0, &b.0));
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

fn is_jpeg_name(name: &str) -> bool {
    if name.starts_with('.') || name.ends_with(".tmp") {
        return false;
    }
    matches!(
        Path::new(name).extension().and_then(|ext| ext.to_str()),
        Some(ext) if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn lists_only_jpegs_in_natural_order() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "b10.jpg",
            "b2.JPG",
            "a.jpeg",
            "notes.txt",
            ".hidden.jpg",
            "x.jpg.tmp",
        ] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let paths = list_jpegs(dir.path()).unwrap();
        assert_eq!(names(&paths), ["a.jpeg", "b2.JPG", "b10.jpg"]);
    }

    #[test]
    fn missing_dir_is_error_with_path() {
        let err = list_jpegs(Path::new("/no/such/dir")).unwrap_err();
        assert!(err.to_string().contains("/no/such/dir"));
        assert!(matches!(err, CoreError::Io { path, .. } if path == Path::new("/no/such/dir")));
    }
}
