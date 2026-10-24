//! Moving a photo to the desktop trash. This is the only place pagecrop takes a photo out of its
//! folder, and it never deletes for good.

use std::path::Path;

use crate::error::CoreError;

/// Moves the photo at `path` to the desktop trash. A path that is not an existing file is an
/// `Io` error with `NotFound`, and the trash is not touched. A failure of the trash itself is a
/// `Trash` error and leaves the photo where it is.
pub fn move_to_trash(path: &Path) -> Result<(), CoreError> {
    if !path.is_file() {
        return Err(CoreError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    }
    trash::delete(path).map_err(|err| CoreError::Trash {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_error_with_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_such_photo.jpg");
        let err = move_to_trash(&path).unwrap_err();
        assert!(
            matches!(&err, CoreError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
        );
        assert!(err.to_string().contains(&path.display().to_string()));
    }
}
