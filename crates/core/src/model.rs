//! The folder being edited: its pictures and, in later rows, their settings.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::decode::read_taken;
use crate::error::CoreError;
use crate::scan::list_jpegs;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageEntry {
    pub file_name: String,
    pub path: PathBuf,
    /// Seconds since the epoch: when the picture was taken, else its modification time.
    pub taken: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Name,
    Date,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub dir: PathBuf,
    /// Pictures in display order.
    pub entries: Vec<ImageEntry>,
    pub sort: SortBy,
}

impl Project {
    /// Opens a folder of pictures, sorted by name. This is the only place a `Project` is built.
    pub fn open(dir: &Path) -> Result<Project, CoreError> {
        let entries = list_jpegs(dir)?
            .into_iter()
            .map(|path| ImageEntry {
                file_name: file_name_of(&path),
                taken: read_taken(&path),
                path,
            })
            .collect();
        Ok(Project {
            dir: dir.to_path_buf(),
            entries,
            sort: SortBy::Name,
        })
    }

    /// Puts the pictures in name order or in date order. Pictures without a date go last in
    /// date order. Ties are broken by name.
    pub fn sort(&mut self, by: SortBy) {
        match by {
            SortBy::Name => self.entries.sort_by(by_name),
            SortBy::Date => self.entries.sort_by(by_date),
        }
        self.sort = by;
    }
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn by_name(a: &ImageEntry, b: &ImageEntry) -> Ordering {
    natord::compare_ignore_case(&a.file_name, &b.file_name)
}

fn by_date(a: &ImageEntry, b: &ImageEntry) -> Ordering {
    date_key(a).cmp(&date_key(b)).then_with(|| by_name(a, b))
}

/// Pictures with a date sort by it; pictures without one sort after all of them.
fn date_key(entry: &ImageEntry) -> (bool, i64) {
    (entry.taken.is_none(), entry.taken.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;

    fn names(project: &Project) -> Vec<&str> {
        project
            .entries
            .iter()
            .map(|e| e.file_name.as_str())
            .collect()
    }

    /// Three empty pictures whose modification times are 3000, 1000 and 2000 seconds after the epoch.
    fn dated_folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, secs) in [("a.jpg", 3000), ("b.jpg", 1000), ("c.jpg", 2000)] {
            let path = dir.path().join(name);
            std::fs::write(&path, b"").unwrap();
            filetime::set_file_mtime(&path, FileTime::from_unix_time(secs, 0)).unwrap();
        }
        dir
    }

    #[test]
    fn date_sort_follows_mtime() {
        let dir = dated_folder();
        let mut project = Project::open(dir.path()).unwrap();
        project.sort(SortBy::Date);
        assert_eq!(names(&project), ["b.jpg", "c.jpg", "a.jpg"]);
    }

    #[test]
    fn name_sort_restores_order() {
        let dir = dated_folder();
        let mut project = Project::open(dir.path()).unwrap();
        project.sort(SortBy::Date);
        project.sort(SortBy::Name);
        assert_eq!(names(&project), ["a.jpg", "b.jpg", "c.jpg"]);
    }

    #[test]
    fn ties_fall_back_to_name() {
        let entry = |name: &str, taken| ImageEntry {
            file_name: name.to_owned(),
            path: PathBuf::from(name),
            taken,
        };
        let mut project = Project {
            dir: PathBuf::new(),
            entries: vec![
                entry("b.jpg", None),
                entry("c.jpg", Some(5)),
                entry("a.jpg", Some(5)),
                entry("d.jpg", Some(1)),
            ],
            sort: SortBy::Name,
        };
        project.sort(SortBy::Date);
        assert_eq!(names(&project), ["d.jpg", "a.jpg", "c.jpg", "b.jpg"]);
    }
}
