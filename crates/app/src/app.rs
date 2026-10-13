//! The application state and the frame that lays out the panels.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pagecrop_core::crop_edit::Handle;
use pagecrop_core::model::{Adjust, CropRect, Project, SortBy};

use crate::loader::{Loader, PreviewReply, PreviewRequest, Thumbnails};
use crate::{editor, panels};

/// All state of the window.
pub struct App {
    /// The open folder, if any.
    pub project: Option<Project>,
    /// Index of the page on screen. 0 when the folder is empty.
    pub selected: usize,
    /// Goes up each time a folder opens. Thumbnails of an older folder are dropped.
    pub generation: u64,
    /// Like `generation`, and it also goes up on a sort, because that moves pages to new indices.
    preview_generation: u64,
    /// Why the last folder could not be opened. The previous folder stays open.
    pub error: Option<String>,
    /// Set by keyboard moves, by opening a folder and by a sort so the page list scrolls to the selected row.
    pub reveal_selected: bool,
    loader: Loader,
    thumbnails: Thumbnails,
    /// One thumbnail per page of the open folder; None until it arrives, or for good if the picture
    /// could not be decoded.
    pub thumbs: Vec<Option<egui::TextureHandle>>,
    /// The last preview to arrive. Shown only while its index is the selected page.
    pub preview: Option<Preview>,
    /// The (generation, selected) pair a preview was last requested for.
    last_requested: Option<(u64, usize)>,
    /// A sort waiting for the user to confirm it, because pages have their own settings.
    pending_sort: Option<SortBy>,
    /// The crop-frame drag in progress, if any.
    pub drag: Option<Drag>,
    /// Whether a grid is drawn over the crop frame.
    pub show_grid: bool,
}

/// A preview of one page.
pub struct Preview {
    pub index: usize,
    pub content: Content,
}

pub enum Content {
    Picture {
        texture: egui::TextureHandle,
        /// Size of the whole picture before shrinking, in pixels.
        full_size: (u32, u32),
    },
    Failed(String),
}

/// A drag of one part of the crop frame. `start` is the frame when the drag began and `press` the
/// pointer position at that moment, both in fractions of the base picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drag {
    pub handle: Handle,
    pub start: CropRect,
    pub press: (f32, f32),
}

impl App {
    pub fn new(
        ctx: egui::Context,
        folder: Option<PathBuf>,
    ) -> Result<App, rayon::ThreadPoolBuildError> {
        let mut app = App {
            project: None,
            selected: 0,
            generation: 0,
            preview_generation: 0,
            error: None,
            reveal_selected: false,
            loader: Loader::spawn(ctx.clone()),
            thumbnails: Thumbnails::new(ctx)?,
            thumbs: Vec::new(),
            preview: None,
            last_requested: None,
            pending_sort: None,
            drag: None,
            show_grid: false,
        };
        if let Some(dir) = folder {
            app.open_folder(&dir);
        }
        Ok(app)
    }

    pub fn open_folder(&mut self, dir: &Path) {
        match Project::open(dir) {
            Ok(project) => {
                self.generation += 1;
                self.preview_generation += 1;
                for entry in &project.entries {
                    self.thumbnails.request(
                        self.generation,
                        entry.file_name.clone(),
                        entry.path.clone(),
                    );
                }
                self.thumbs = vec![None; project.entries.len()];
                self.project = Some(project);
                self.selected = 0;
                self.reveal_selected = true;
                self.preview = None;
                self.pending_sort = None;
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

    /// Takes the finished previews that belong to the page on screen.
    fn take_replies(&mut self, ctx: &egui::Context) {
        while let Some(reply) = self.loader.poll() {
            match reply {
                PreviewReply::Loaded {
                    index,
                    generation,
                    image,
                    full_size,
                } if self.is_current(index, generation) => {
                    let texture = ctx.load_texture("preview", image, egui::TextureOptions::LINEAR);
                    self.preview = Some(Preview {
                        index,
                        content: Content::Picture { texture, full_size },
                    });
                }
                PreviewReply::Failed {
                    index,
                    generation,
                    message,
                } if self.is_current(index, generation) => {
                    self.preview = Some(Preview {
                        index,
                        content: Content::Failed(message),
                    });
                }
                _ => {}
            }
        }
    }

    /// What to show for the selected page, once its preview has arrived.
    pub fn selected_content(&self) -> Option<&Content> {
        let preview = self.preview.as_ref()?;
        (preview.index == self.selected).then_some(&preview.content)
    }

    /// Stores the thumbnails of the open folder as they arrive.
    fn take_thumbs(&mut self, ctx: &egui::Context) {
        while let Some(thumb) = self.thumbnails.poll() {
            if thumb.generation != self.generation {
                continue;
            }
            let (Some(image), Some(project)) = (thumb.image, self.project.as_ref()) else {
                continue;
            };
            let Some(index) = project
                .entries
                .iter()
                .position(|e| e.file_name == thumb.file_name)
            else {
                continue;
            };
            let texture = ctx.load_texture(
                format!("thumb {}", thumb.file_name),
                image,
                egui::TextureOptions::LINEAR,
            );
            if let Some(slot) = self.thumbs.get_mut(index) {
                *slot = Some(texture);
            }
        }
    }

    /// The settings that apply to the selected page.
    pub fn effective(&self) -> Adjust {
        self.project
            .as_ref()
            .map_or_else(Adjust::default, |p| p.effective(self.selected))
    }

    /// Gives the selected page its own settings. Pages after it follow them.
    pub fn apply_edit(&mut self, adjust: Adjust) {
        let selected = self.selected;
        if let Some(project) = self.project.as_mut() {
            project.set_key(selected, adjust);
        }
    }

    /// Removes the own settings of the selected page.
    pub fn reset_page(&mut self) {
        self.begin_edit();
        let selected = self.selected;
        if let Some(project) = self.project.as_mut() {
            project.clear_key(selected);
        }
        self.commit_edit();
    }

    pub fn page_has_key(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            p.entries
                .get(self.selected)
                .is_some_and(|e| e.key.is_some())
        })
    }

    /// What the Page section says about the selected page.
    pub fn page_label(&self) -> String {
        let Some(project) = &self.project else {
            return String::new();
        };
        if self.page_has_key() {
            return "Own settings".to_owned();
        }
        match project.key_index(self.selected) {
            Some(k) => format!("Follows page {}", k + 1),
            None => "Default settings".to_owned(),
        }
    }

    /// Starts an edit. The undo history comes in row 0001.12.
    pub fn begin_edit(&mut self) {}

    /// Finishes an edit. Saving the settings comes in row 0001.10.
    pub fn commit_edit(&mut self) {}

    /// Sorts now, or asks first when pages have their own settings.
    pub fn request_sort(&mut self, by: SortBy) {
        let Some(project) = &self.project else {
            return;
        };
        if project.sort == by {
            return;
        }
        if project.entries.iter().any(|e| e.key.is_some()) {
            self.pending_sort = Some(by);
        } else {
            self.set_sort(by);
        }
    }

    /// The question shown before a sort that would change which pages follow which.
    fn confirm_sort(&mut self, ctx: &egui::Context) {
        let Some(by) = self.pending_sort else {
            return;
        };
        let mut answer = None;
        egui::Window::new("Sort order")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Sort order decides which pages follow which. Switch anyway?");
                ui.horizontal(|ui| {
                    if ui.button("Switch").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.pending_sort = None;
                self.set_sort(by);
            }
            Some(false) => self.pending_sort = None,
            None => {}
        }
    }

    /// Sorts the folder by name or by date. The selected page stays selected, and the
    /// thumbnails move with their pages.
    pub fn set_sort(&mut self, by: SortBy) {
        let selected_name = self
            .project
            .as_ref()
            .and_then(|p| p.entries.get(self.selected))
            .map(|e| e.file_name.clone());
        let Some(project) = self.project.as_mut() else {
            return;
        };
        if project.sort == by {
            return;
        }
        let mut thumbs_by_name: HashMap<String, egui::TextureHandle> = project
            .entries
            .iter()
            .zip(self.thumbs.iter())
            .filter_map(|(e, t)| t.clone().map(|t| (e.file_name.clone(), t)))
            .collect();
        project.sort(by);
        self.thumbs = project
            .entries
            .iter()
            .map(|e| thumbs_by_name.remove(&e.file_name))
            .collect();
        self.selected = selected_name
            .and_then(|name| project.entries.iter().position(|e| e.file_name == name))
            .unwrap_or(0);
        self.reveal_selected = true;
        // Previews still on their way were asked for by old index, so they are dropped.
        self.preview_generation += 1;
        self.preview = None;
        self.last_requested = None;
    }

    fn is_current(&self, index: usize, generation: u64) -> bool {
        generation == self.preview_generation && index == self.selected
    }

    /// Asks the worker for the selected page whenever the folder or the selection has changed.
    fn request_preview_if_changed(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let key = (self.preview_generation, self.selected);
        if self.last_requested == Some(key) {
            return;
        }
        self.last_requested = Some(key);
        if let Some(entry) = project.entries.get(self.selected) {
            self.loader.request(PreviewRequest {
                index: self.selected,
                generation: self.preview_generation,
                path: entry.path.clone(),
            });
        }
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
        self.take_replies(ui.ctx());
        self.take_thumbs(ui.ctx());
        self.request_preview_if_changed();
        self.confirm_sort(ui.ctx());
        panels::top::show(self, ui);
        panels::status::show(self, ui);
        panels::pages::show(self, ui);
        panels::adjust::show(self, ui);
        editor::show(self, ui);
    }
}
