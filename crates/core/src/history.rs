//! Undo and redo for changes to the settings of the pages.

use crate::model::{Adjust, Project};

/// The most edits kept for undo. The oldest one is dropped when there are more.
const MAX_EDITS: usize = 200;

/// One change to the own settings of one page: what the page had before and what it has now.
/// `None` means the page had no settings of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    /// The page's index in the project.
    pub index: usize,
    pub before: Option<Adjust>,
    pub after: Option<Adjust>,
}

/// The edits that can be undone and the ones that can be redone.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl History {
    /// Records an edit. An edit that changes nothing is not recorded. A new edit makes the redo
    /// list empty.
    pub fn push(&mut self, edit: Edit) {
        if edit.before == edit.after {
            return;
        }
        self.redo.clear();
        self.undo.push(edit);
        if self.undo.len() > MAX_EDITS {
            self.undo.remove(0);
        }
    }

    /// Records an edit like `push`, except that when it continues the last edit on the same page,
    /// the two become one step. A run of nudges on one page then undoes in one go.
    pub fn extend(&mut self, edit: Edit) {
        let continues = self
            .undo
            .last()
            .is_some_and(|last| last.index == edit.index && last.after == edit.before);
        if !continues {
            self.push(edit);
            return;
        }
        self.redo.clear();
        if let Some(last) = self.undo.last_mut() {
            last.after = edit.after;
        }
        if self
            .undo
            .last()
            .is_some_and(|last| last.before == last.after)
        {
            self.undo.pop();
        }
    }

    /// Takes back the most recent edit. Returns the index of the page it changed, or None when
    /// there is nothing to undo or the page no longer exists.
    pub fn undo(&mut self, project: &mut Project) -> Option<usize> {
        let edit = self.undo.pop()?;
        restore(project, edit.index, edit.before)?;
        self.redo.push(edit);
        Some(edit.index)
    }

    /// Applies the most recently undone edit again. Returns the index of the page it changed, or
    /// None when there is nothing to redo or the page no longer exists.
    pub fn redo(&mut self, project: &mut Project) -> Option<usize> {
        let edit = self.redo.pop()?;
        restore(project, edit.index, edit.after)?;
        self.undo.push(edit);
        Some(edit.index)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forgets every edit, for example when another folder is opened.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Gives page `index` the settings `key`, or none. Returns the index, or None when the page does
/// not exist.
fn restore(project: &mut Project, index: usize, key: Option<Adjust>) -> Option<usize> {
    if index >= project.entries.len() {
        return None;
    }
    match key {
        Some(adjust) => project.set_key(index, adjust),
        None => project.clear_key(index),
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of twelve empty pictures, opened with no settings.
    fn pages() -> Project {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=12 {
            std::fs::write(dir.path().join(format!("page_{n:03}.jpg")), b"").unwrap();
        }
        Project::open_without_settings(dir.path()).unwrap()
    }

    /// Settings that differ from the default only in brightness, so keys can be told apart.
    fn brightness(value: i32) -> Option<Adjust> {
        Some(Adjust {
            brightness: value,
            ..Adjust::default()
        })
    }

    fn edit(index: usize, before: Option<Adjust>, after: Option<Adjust>) -> Edit {
        Edit {
            index,
            before,
            after,
        }
    }

    #[test]
    fn undo_restores_previous_key() {
        let mut project = pages();
        let mut history = History::default();
        project.set_key(3, brightness(10).unwrap());
        history.push(edit(3, brightness(10), brightness(20)));
        project.set_key(3, brightness(20).unwrap());
        assert_eq!(history.undo(&mut project), Some(3));
        assert_eq!(project.entries[3].key, brightness(10));
    }

    #[test]
    fn undo_of_first_key_removes_it() {
        let mut project = pages();
        let mut history = History::default();
        project.set_key(1, brightness(5).unwrap());
        project.set_key(3, brightness(10).unwrap());
        history.push(edit(3, None, brightness(10)));
        assert_eq!(history.undo(&mut project), Some(3));
        assert_eq!(project.entries[3].key, None);
        assert_eq!(project.effective(3).brightness, 5);
    }

    #[test]
    fn redo_reapplies() {
        let mut project = pages();
        let mut history = History::default();
        history.push(edit(3, None, brightness(10)));
        project.set_key(3, brightness(10).unwrap());
        assert_eq!(history.undo(&mut project), Some(3));
        assert_eq!(project.entries[3].key, None);
        assert_eq!(history.redo(&mut project), Some(3));
        assert_eq!(project.entries[3].key, brightness(10));
        assert!(!history.can_redo());
    }

    #[test]
    fn push_after_undo_clears_redo() {
        let mut project = pages();
        let mut history = History::default();
        history.push(edit(3, None, brightness(10)));
        history.undo(&mut project);
        assert!(history.can_redo());
        history.push(edit(5, None, brightness(20)));
        assert!(!history.can_redo());
        assert_eq!(history.redo(&mut project), None);
    }

    #[test]
    fn no_op_edit_is_not_recorded() {
        let mut project = pages();
        let mut history = History::default();
        history.push(edit(3, brightness(10), brightness(10)));
        assert!(!history.can_undo());
        assert_eq!(history.undo(&mut project), None);
    }

    #[test]
    fn consecutive_edits_on_one_page_undo_together() {
        let mut project = pages();
        let mut history = History::default();
        project.set_key(3, brightness(10).unwrap());
        history.extend(edit(3, None, brightness(10)));
        project.set_key(3, brightness(20).unwrap());
        history.extend(edit(3, brightness(10), brightness(20)));
        assert_eq!(history.undo(&mut project), Some(3));
        assert_eq!(project.entries[3].key, None);
        assert!(!history.can_undo());
    }

    #[test]
    fn history_is_capped_at_200() {
        let mut project = pages();
        let mut history = History::default();
        project.set_key(0, brightness(201).unwrap());
        for value in 0..201 {
            history.push(edit(0, brightness(value), brightness(value + 1)));
        }
        let mut undone = 0;
        while history.undo(&mut project).is_some() {
            undone += 1;
        }
        assert_eq!(undone, 200);
        assert_eq!(project.entries[0].key, brightness(1));
    }
}
