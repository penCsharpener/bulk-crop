//! The folder being edited: its pictures and the settings that flow from page to page.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::decode::read_taken;
use crate::error::CoreError;
use crate::persist::load;
use crate::scan::list_jpegs;

/// A crop frame in fractions of the picture: the centre (`cx`, `cy`) and the size (`w`, `h`).
/// The default is the whole picture.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CropRect {
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for CropRect {
    fn default() -> Self {
        CropRect {
            cx: 0.0,
            cy: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }
}

/// Everything one page can change. The default changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjust {
    /// Quarter turns clockwise, 0 to 3.
    pub quarter_turns: u8,
    /// Fine angle in degrees, -45 to 45. Positive turns clockwise on screen.
    pub angle_deg: f32,
    pub crop: CropRect,
    /// -100 to 100.
    pub brightness: i32,
    /// -100 to 100.
    pub contrast: i32,
    pub grayscale: bool,
}

impl Default for Adjust {
    fn default() -> Self {
        Adjust {
            quarter_turns: 0,
            angle_deg: 0.0,
            crop: CropRect::default(),
            brightness: 0,
            contrast: 0,
            grayscale: false,
        }
    }
}

impl Adjust {
    /// Brings every field into its allowed range.
    pub fn normalized(self) -> Adjust {
        Adjust {
            quarter_turns: self.quarter_turns % 4,
            angle_deg: self.angle_deg.clamp(-45.0, 45.0),
            crop: CropRect {
                w: self.crop.w.max(0.02),
                h: self.crop.h.max(0.02),
                ..self.crop
            },
            brightness: self.brightness.clamp(-100, 100),
            contrast: self.contrast.clamp(-100, 100),
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageEntry {
    pub file_name: String,
    pub path: PathBuf,
    /// Seconds since the epoch: when the picture was taken, else its modification time.
    pub taken: Option<i64>,
    /// The settings of this page, if it has its own.
    pub key: Option<Adjust>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    Name,
    Date,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub dir: PathBuf,
    /// Pictures in display order.
    pub entries: Vec<ImageEntry>,
    pub sort: SortBy,
    /// Saved settings whose picture is no longer in the folder. They are kept when saving.
    pub orphans: BTreeMap<String, Adjust>,
}

impl Project {
    /// Opens a folder of pictures with the settings saved in it. This is the only place a `Project`
    /// is built.
    pub fn open(dir: &Path) -> Result<Project, CoreError> {
        let (sort, mut keys) = match load(dir)? {
            Some(saved) => (saved.sort, saved.keys),
            None => (SortBy::Name, BTreeMap::new()),
        };
        let entries = list_jpegs(dir)?
            .into_iter()
            .map(|path| {
                let file_name = file_name_of(&path);
                ImageEntry {
                    key: keys.remove(&file_name),
                    taken: read_taken(&path),
                    file_name,
                    path,
                }
            })
            .collect();
        let mut project = Project {
            dir: dir.to_path_buf(),
            entries,
            sort: SortBy::Name,
            orphans: keys,
        };
        project.sort(sort);
        Ok(project)
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

    /// The index of the page whose settings page `i` uses: the nearest page at or before `i`
    /// that has its own. None when there is none, or when `i` is out of range.
    pub fn key_index(&self, i: usize) -> Option<usize> {
        if i >= self.entries.len() {
            return None;
        }
        (0..=i).rev().find(|&k| self.has_key(k))
    }

    /// The settings that apply to page `i`. The default when none applies or `i` is out of range.
    pub fn effective(&self, i: usize) -> Adjust {
        self.key_index(i)
            .and_then(|k| self.entries.get(k))
            .and_then(|entry| entry.key)
            .unwrap_or_default()
    }

    /// Gives page `i` its own settings, stored in normalized form.
    pub fn set_key(&mut self, i: usize, a: Adjust) {
        if let Some(entry) = self.entries.get_mut(i) {
            entry.key = Some(a.normalized());
        }
    }

    /// Removes the own settings of page `i`, so it follows the page before it again.
    pub fn clear_key(&mut self, i: usize) {
        if let Some(entry) = self.entries.get_mut(i) {
            entry.key = None;
        }
    }

    /// The pages that use the settings of page `i`: from `i` up to the next page with its own
    /// settings, or the end. Empty when `i` is out of range.
    pub fn span(&self, i: usize) -> Range<usize> {
        let len = self.entries.len();
        if i >= len {
            return i..i;
        }
        let end = (i + 1..len).find(|&k| self.has_key(k)).unwrap_or(len);
        i..end
    }

    fn has_key(&self, i: usize) -> bool {
        self.entries.get(i).is_some_and(|e| e.key.is_some())
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
            key: None,
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
            orphans: BTreeMap::new(),
        };
        project.sort(SortBy::Date);
        assert_eq!(names(&project), ["d.jpg", "a.jpg", "c.jpg", "b.jpg"]);
    }

    /// A project of `count` pages with no settings.
    fn pages(count: usize) -> Project {
        Project {
            dir: PathBuf::new(),
            entries: (1..=count)
                .map(|n| {
                    let name = format!("page_{n:03}.jpg");
                    ImageEntry {
                        path: PathBuf::from(&name),
                        file_name: name,
                        taken: None,
                        key: None,
                    }
                })
                .collect(),
            sort: SortBy::Name,
            orphans: BTreeMap::new(),
        }
    }

    /// Settings that differ from the default only in brightness, so keys can be told apart.
    fn brightness(value: i32) -> Adjust {
        Adjust {
            brightness: value,
            ..Adjust::default()
        }
    }

    /// The brightness that page `page` uses. Page numbers start at 1, as on screen.
    fn seen(project: &Project, page: usize) -> i32 {
        project.effective(page - 1).brightness
    }

    #[test]
    fn no_keys_gives_default_everywhere() {
        let project = pages(12);
        for page in 1..=12 {
            assert_eq!(project.effective(page - 1), Adjust::default());
        }
    }

    #[test]
    fn key_on_4_applies_to_4_through_12() {
        let mut project = pages(12);
        project.set_key(3, brightness(7));
        for page in 1..=3 {
            assert_eq!(seen(&project, page), 0, "page {page}");
        }
        for page in 4..=12 {
            assert_eq!(seen(&project, page), 7, "page {page}");
        }
    }

    #[test]
    fn second_key_on_8_ends_first_span() {
        let mut project = pages(12);
        project.set_key(3, brightness(10));
        project.set_key(7, brightness(20));
        for page in 4..=7 {
            assert_eq!(seen(&project, page), 10, "page {page}");
        }
        for page in 8..=12 {
            assert_eq!(seen(&project, page), 20, "page {page}");
        }
    }

    #[test]
    fn key_on_6_between_two_keys_stops_before_8() {
        let mut project = pages(12);
        project.set_key(3, brightness(10));
        project.set_key(7, brightness(20));
        project.set_key(5, brightness(30));
        assert_eq!(seen(&project, 4), 10);
        assert_eq!(seen(&project, 5), 10);
        assert_eq!(seen(&project, 6), 30);
        assert_eq!(seen(&project, 7), 30);
        for page in 8..=12 {
            assert_eq!(seen(&project, page), 20, "page {page}");
        }
    }

    #[test]
    fn clear_key_falls_back_to_previous() {
        let mut project = pages(12);
        project.set_key(3, brightness(10));
        project.set_key(7, brightness(20));
        project.clear_key(7);
        for page in 8..=12 {
            assert_eq!(seen(&project, page), 10, "page {page}");
        }
    }

    #[test]
    fn span_of_4_is_4_to_8() {
        let mut project = pages(12);
        project.set_key(3, brightness(10));
        project.set_key(7, brightness(20));
        assert_eq!(project.span(3), 3..7);
    }

    #[test]
    fn normalized_clamps_all_fields() {
        let a = Adjust {
            quarter_turns: 5,
            angle_deg: 50.0,
            crop: CropRect {
                cx: 0.3,
                cy: 0.4,
                w: 0.0,
                h: 0.001,
            },
            brightness: 150,
            contrast: -150,
            grayscale: true,
        }
        .normalized();
        assert_eq!(a.quarter_turns, 1);
        assert!((a.angle_deg - 45.0).abs() < 1e-4);
        assert!((a.crop.w - 0.02).abs() < 1e-4);
        assert!((a.crop.h - 0.02).abs() < 1e-4);
        assert!((a.crop.cx - 0.3).abs() < 1e-4);
        assert_eq!(a.brightness, 100);
        assert_eq!(a.contrast, -100);
        assert!(a.grayscale);
    }

    #[test]
    fn out_of_range_index_is_harmless() {
        let mut project = pages(12);
        assert_eq!(project.effective(99), Adjust::default());
        project.set_key(99, brightness(5));
        project.clear_key(99);
        assert_eq!(project.key_index(99), None);
        assert_eq!(project.span(99), 99..99);
        assert!(project.entries.iter().all(|e| e.key.is_none()));
    }
}
