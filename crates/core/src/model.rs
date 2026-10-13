//! The folder being edited: its pictures and, in later rows, their settings.

use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::scan::list_jpegs;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageEntry {
    pub file_name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub dir: PathBuf,
    /// Pictures in display order.
    pub entries: Vec<ImageEntry>,
}

impl Project {
    /// Opens a folder of pictures. This is the only place a `Project` is built.
    pub fn open(dir: &Path) -> Result<Project, CoreError> {
        let entries = list_jpegs(dir)?
            .into_iter()
            .map(|path| ImageEntry {
                file_name: file_name_of(&path),
                path,
            })
            .collect();
        Ok(Project {
            dir: dir.to_path_buf(),
            entries,
        })
    }
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
