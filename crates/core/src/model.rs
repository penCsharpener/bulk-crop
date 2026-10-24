//! The folder being edited: its pictures and the settings that flow from page to page.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::decode::read_taken;
use crate::error::CoreError;
use crate::export::ExportOptions;
use crate::geometry::turn_crop;
use crate::persist::{Saved, load};
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
    /// From this page on, odd and even pages keep their own frame and fine angle.
    pub alternate: bool,
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
            alternate: false,
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
    /// Where and how the pictures are exported.
    pub export: ExportOptions,
}

impl Project {
    /// Opens a folder of pictures with the settings saved in it.
    pub fn open(dir: &Path) -> Result<Project, CoreError> {
        let saved = load(dir)?;
        Project::build(dir, saved)
    }

    /// Opens a folder of pictures without its saved settings, for when the settings file cannot be
    /// read.
    pub fn open_without_settings(dir: &Path) -> Result<Project, CoreError> {
        Project::build(dir, None)
    }

    /// The only place a `Project` is built.
    fn build(dir: &Path, saved: Option<Saved>) -> Result<Project, CoreError> {
        let (sort, mut keys, saved_export) = match saved {
            Some(saved) => (saved.sort, saved.keys, saved.export),
            None => (SortBy::Name, BTreeMap::new(), None),
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
        let export = saved_export.unwrap_or_else(|| ExportOptions::for_folder(dir));
        let mut project = Project {
            dir: dir.to_path_buf(),
            entries,
            sort: SortBy::Name,
            orphans: keys,
            export,
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
    /// Inside a run of alternating keys the frame and the fine angle come from the nearest key on
    /// the same side; everything else comes from the nearest key.
    pub fn effective(&self, i: usize) -> Adjust {
        let Some(k) = self.key_index(i) else {
            return Adjust::default();
        };
        let Some(own) = self.key_at(k) else {
            return Adjust::default();
        };
        if !own.alternate {
            return own;
        }
        let anchor = self.run_start(k);
        let side = self.key_at(self.side_source(anchor, i)).unwrap_or(own);
        Adjust {
            angle_deg: side.angle_deg,
            crop: turn_crop(side.crop, side.quarter_turns, own.quarter_turns),
            ..own
        }
    }

    /// Which side page `i` is on inside a run of alternating keys: 0 for the side of the run's first
    /// page, 1 for the other. None when page `i` does not alternate or is out of range.
    pub fn side(&self, i: usize) -> Option<u8> {
        let k = self.key_index(i)?;
        if !self.key_at(k)?.alternate {
            return None;
        }
        let anchor = self.run_start(k);
        Some((i.saturating_sub(anchor) % 2) as u8)
    }

    fn key_at(&self, i: usize) -> Option<Adjust> {
        self.entries.get(i).and_then(|e| e.key)
    }

    /// The first key of the run of alternating keys that ends at key `k`.
    fn run_start(&self, k: usize) -> usize {
        let mut start = k;
        while let Some(prev) = self.keyed_page_from(start, false) {
            if !self.key_at(prev).is_some_and(|a| a.alternate) {
                break;
            }
            start = prev;
        }
        start
    }

    /// The page whose frame and fine angle page `i` uses inside the run that starts at `anchor`: the
    /// nearest page at or before `i` on the same side that has its own settings, else the anchor.
    fn side_source(&self, anchor: usize, i: usize) -> usize {
        let parity = i.saturating_sub(anchor) % 2;
        (anchor..=i)
            .rev()
            .filter(|j| (j - anchor) % 2 == parity)
            .find(|&j| self.has_key(j))
            .unwrap_or(anchor)
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

    /// The nearest page before or after `from` that has its own settings, searching forward or
    /// backward. None when there is no such page.
    pub fn keyed_page_from(&self, from: usize, forward: bool) -> Option<usize> {
        if forward {
            (from + 1..self.entries.len()).find(|&i| self.has_key(i))
        } else {
            (0..from.min(self.entries.len()))
                .rev()
                .find(|&i| self.has_key(i))
        }
    }

    /// Takes page `i` out of the list and returns it. When the page had its own settings and the page
    /// after it has none, the settings move to that page, so the pages that followed it look the same.
    /// None when `i` is out of range.
    pub fn remove_page(&mut self, i: usize) -> Option<ImageEntry> {
        if i >= self.entries.len() {
            return None;
        }
        let removed = self.entries.remove(i);
        if let (Some(key), Some(next)) = (removed.key, self.entries.get_mut(i))
            && next.key.is_none()
        {
            next.key = Some(key);
        }
        Some(removed)
    }

    fn has_key(&self, i: usize) -> bool {
        self.entries.get(i).is_some_and(|e| e.key.is_some())
    }
}

/// The pages whose previews are wanted, in the order they are needed: the selected page, the two
/// after it, then the one before. Only pages inside `0..count` are listed.
pub fn read_ahead_order(selected: usize, count: usize) -> Vec<usize> {
    let before = selected.checked_sub(1);
    [
        Some(selected),
        Some(selected + 1),
        Some(selected + 2),
        before,
    ]
    .into_iter()
    .flatten()
    .filter(|&i| i < count)
    .collect()
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
    use crate::geometry::{quarter_turn, turn_crop};
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
            export: ExportOptions::default(),
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
            export: ExportOptions::default(),
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
            alternate: false,
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
    fn keyed_page_steps_between_own_settings() {
        let mut project = pages(12);
        project.set_key(3, brightness(10));
        project.set_key(7, brightness(20));
        assert_eq!(project.keyed_page_from(3, true), Some(7));
        assert_eq!(project.keyed_page_from(7, false), Some(3));
        assert_eq!(project.keyed_page_from(0, false), None);
        assert_eq!(project.keyed_page_from(7, true), None);
    }

    #[test]
    fn read_ahead_order_is_selected_then_two_after_then_one_before() {
        assert_eq!(read_ahead_order(4, 12), vec![4, 5, 6, 3]);
        assert_eq!(read_ahead_order(0, 12), vec![0, 1, 2]);
        assert_eq!(read_ahead_order(11, 12), vec![11, 10]);
        assert_eq!(read_ahead_order(0, 0), Vec::<usize>::new());
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

    /// A key that alternates, with a frame whose `cx` tells it apart from other frames.
    fn frame(cx: f32) -> Adjust {
        Adjust {
            alternate: true,
            crop: CropRect {
                cx,
                cy: 0.0,
                w: 0.5,
                h: 0.5,
            },
            ..Adjust::default()
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// The frame and the fine angle that page `page` uses. Page numbers start at 1.
    fn look(project: &Project, page: usize) -> (f32, f32) {
        let a = project.effective(page - 1);
        (a.crop.cx, a.angle_deg)
    }

    fn assert_look(project: &Project, page: usize, cx: f32, angle: f32) {
        let (got_cx, got_angle) = look(project, page);
        assert!(
            close(got_cx, cx) && close(got_angle, angle),
            "page {page}: got ({got_cx}, {got_angle}), wanted ({cx}, {angle})"
        );
    }

    /// Twelve pages; page 3 turns alternating on with frame A (cx 0.1, angle 1) and page 4 has its
    /// own side-B frame (cx -0.1, angle -1).
    fn two_sided() -> Project {
        let mut project = pages(12);
        project.set_key(
            2,
            Adjust {
                angle_deg: 1.0,
                ..frame(0.1)
            },
        );
        project.set_key(
            3,
            Adjust {
                angle_deg: -1.0,
                ..frame(-0.1)
            },
        );
        project
    }

    #[test]
    fn alternating_pages_use_two_frames() {
        let project = two_sided();
        for page in [3, 5, 7, 9, 11] {
            assert_look(&project, page, 0.1, 1.0);
        }
        for page in [4, 6, 8, 10, 12] {
            assert_look(&project, page, -0.1, -1.0);
        }
    }

    #[test]
    fn side_b_starts_as_copy_of_side_a() {
        let mut project = pages(12);
        project.set_key(2, frame(0.1));
        for page in 3..=12 {
            assert_look(&project, page, 0.1, 0.0);
        }
    }

    #[test]
    fn tone_on_one_side_flows_to_both() {
        let mut project = two_sided();
        let key = Adjust {
            brightness: 30,
            ..project.effective(5)
        };
        project.set_key(5, key);
        assert_eq!(seen(&project, 7), 30);
        assert_look(&project, 7, 0.1, 1.0);
        assert_eq!(seen(&project, 8), 30);
        assert_look(&project, 8, -0.1, -1.0);
    }

    #[test]
    fn editing_side_a_leaves_side_b() {
        let mut project = two_sided();
        let key = Adjust {
            crop: frame(0.3).crop,
            ..project.effective(6)
        };
        project.set_key(6, key);
        for page in [7, 9, 11] {
            assert_look(&project, page, 0.3, 1.0);
        }
        for page in [8, 10, 12] {
            assert_look(&project, page, -0.1, -1.0);
        }
    }

    #[test]
    fn turning_off_keeps_earlier_pages_alternating() {
        let mut project = two_sided();
        let key = Adjust {
            alternate: false,
            ..project.effective(9)
        };
        project.set_key(9, key);
        for page in [3, 5, 7, 9] {
            assert_look(&project, page, 0.1, 1.0);
        }
        for page in [4, 6, 8] {
            assert_look(&project, page, -0.1, -1.0);
        }
        for page in 10..=12 {
            assert_look(&project, page, -0.1, -1.0);
        }
        assert_eq!(project.side(9), None);
    }

    #[test]
    fn side_frames_follow_quarter_turn() {
        let mut project = pages(12);
        let a = Adjust {
            crop: CropRect {
                cx: 0.2,
                cy: 0.0,
                w: 0.5,
                h: 0.9,
            },
            ..frame(0.2)
        };
        project.set_key(2, a);
        let turned = quarter_turn(project.effective(4), true);
        project.set_key(4, turned);
        let got = project.effective(5);
        let want = turn_crop(a.crop, 0, 1);
        assert_eq!(got.quarter_turns, 1);
        assert!(close(got.crop.cx, want.cx) && close(got.crop.cy, want.cy));
        assert!(close(got.crop.w, want.w) && close(got.crop.h, want.h));
    }

    #[test]
    fn side_of_page() {
        let project = two_sided();
        let sides: Vec<Option<u8>> = (2..=5).map(|page| project.side(page - 1)).collect();
        assert_eq!(sides, [None, Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn pages_without_alternate_behave_as_before() {
        let key = |value| Adjust {
            alternate: false,
            ..brightness(value)
        };
        let mut project = pages(12);
        project.set_key(3, key(10));
        project.set_key(7, key(20));
        project.set_key(5, key(30));
        assert_eq!(seen(&project, 4), 10);
        assert_eq!(seen(&project, 5), 10);
        assert_eq!(seen(&project, 6), 30);
        assert_eq!(seen(&project, 7), 30);
        for page in 8..=12 {
            assert_eq!(seen(&project, page), 20, "page {page}");
        }
        assert_eq!(project.side(5), None);
    }

    /// Gives pages keys that differ only in brightness. Each pair is a page number from 1 and a value.
    fn key_pages(project: &mut Project, keys: &[(usize, i32)]) {
        for &(page, value) in keys {
            project.set_key(page - 1, brightness(value));
        }
    }

    #[test]
    fn removing_keyed_page_moves_key_to_next() {
        let mut project = pages(12);
        key_pages(&mut project, &[(4, 7)]);
        let removed = project.remove_page(3).unwrap();
        assert_eq!(removed.file_name, "page_004.jpg");
        assert_eq!(project.entries.len(), 11);
        assert_eq!(project.entries[3].file_name, "page_005.jpg");
        assert_eq!(project.entries[3].key, Some(brightness(7)));
        for page in 4..=11 {
            assert_eq!(seen(&project, page), 7, "page {page}");
        }
        for page in 1..=3 {
            assert_eq!(seen(&project, page), 0, "page {page}");
        }
    }

    #[test]
    fn removing_keyed_page_before_keyed_page_drops_key() {
        let mut project = pages(12);
        key_pages(&mut project, &[(4, 7), (5, 9)]);
        project.remove_page(3);
        assert_eq!(project.entries[3].file_name, "page_005.jpg");
        assert_eq!(project.entries[3].key, Some(brightness(9)));
        for page in 4..=11 {
            assert_eq!(seen(&project, page), 9, "page {page}");
        }
    }

    #[test]
    fn removing_plain_page_keeps_keys() {
        let mut project = pages(12);
        key_pages(&mut project, &[(3, 5), (8, 6)]);
        project.remove_page(4);
        assert_eq!(project.entries.len(), 11);
        assert_eq!(project.entries[2].key, Some(brightness(5)));
        assert_eq!(project.entries[6].file_name, "page_008.jpg");
        assert_eq!(project.entries[6].key, Some(brightness(6)));
        assert_eq!(
            project.entries.iter().filter(|e| e.key.is_some()).count(),
            2
        );
        assert_eq!(seen(&project, 3), 5);
        assert_eq!(seen(&project, 6), 5);
        assert_eq!(seen(&project, 7), 6);
    }

    #[test]
    fn removing_doubled_shot_restores_sides() {
        let mut project = pages(7);
        project.set_key(0, frame(0.1));
        project.set_key(1, frame(-0.1));
        // Page 4 is a double of page 3, so page 5 wrongly has frame A.
        assert_look(&project, 5, 0.1, 0.0);
        project.remove_page(3);
        assert_look(&project, 4, -0.1, 0.0);
        assert_look(&project, 5, 0.1, 0.0);
    }

    #[test]
    fn remove_out_of_range_is_none() {
        let mut project = pages(3);
        assert!(project.remove_page(3).is_none());
        assert!(project.remove_page(99).is_none());
        assert_eq!(project.entries.len(), 3);
    }
}
