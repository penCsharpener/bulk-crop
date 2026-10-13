//! The application state and the frame that lays out the panels.

use std::path::{Path, PathBuf};

use pagecrop_core::model::Project;

use crate::{editor, panels};

/// All state of the window.
#[derive(Default)]
pub struct App {
    /// The open folder, if any.
    pub project: Option<Project>,
    /// Index of the page on screen. 0 when the folder is empty.
    pub selected: usize,
    /// Goes up each time a folder opens. Results of an older folder are dropped once loading exists.
    pub generation: u64,
    /// Why the last folder could not be opened. The previous folder stays open.
    pub error: Option<String>,
    /// Set by keyboard moves and by opening a folder so the page list scrolls to the selected row.
    pub reveal_selected: bool,
}

impl App {
    pub fn new(folder: Option<PathBuf>) -> App {
        let mut app = App::default();
        if let Some(dir) = folder {
            app.open_folder(&dir);
        }
        app
    }

    pub fn open_folder(&mut self, dir: &Path) {
        match Project::open(dir) {
            Ok(project) => {
                self.project = Some(project);
                self.selected = 0;
                self.reveal_selected = true;
                self.generation += 1;
                self.error = None;
            }
            Err(err) => self.error = Some(err.to_string()),
        }
    }

    pub fn page_count(&self) -> usize {
        self.project.as_ref().map_or(0, |p| p.entries.len())
    }

    /// Selects page `index`, clamped to the last page.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.page_count().saturating_sub(1));
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.page_count() == 0 {
            return;
        }
        let current = self.selected;
        let last = self.page_count().saturating_sub(1);
        let target = ctx.input(|input| key_target(input, current, last));
        if let Some(index) = target {
            self.select(index);
            self.reveal_selected = true;
        }
    }
}

/// The page a navigation key asks for, if one was pressed this frame.
fn key_target(input: &egui::InputState, current: usize, last: usize) -> Option<usize> {
    use egui::Key;
    if input.key_pressed(Key::ArrowDown)
        || input.key_pressed(Key::PageDown)
        || input.key_pressed(Key::ArrowRight)
    {
        Some(current + 1)
    } else if input.key_pressed(Key::ArrowUp)
        || input.key_pressed(Key::PageUp)
        || input.key_pressed(Key::ArrowLeft)
    {
        Some(current.saturating_sub(1))
    } else if input.key_pressed(Key::Home) {
        Some(0)
    } else if input.key_pressed(Key::End) {
        Some(last)
    } else {
        None
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_keys(ui.ctx());
        panels::top::show(self, ui);
        panels::status::show(self, ui);
        panels::pages::show(self, ui);
        panels::adjust::show(ui);
        editor::show(ui);
    }
}
