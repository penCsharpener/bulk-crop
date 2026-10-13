//! The application state and the frame that lays out the panels.

use std::path::{Path, PathBuf};

use pagecrop_core::model::Project;

use crate::loader::{Loader, PreviewReply, PreviewRequest, Thumbnails};
use crate::{editor, panels};

/// All state of the window.
pub struct App {
    /// The open folder, if any.
    pub project: Option<Project>,
    /// Index of the page on screen. 0 when the folder is empty.
    pub selected: usize,
    /// Goes up each time a folder opens. Previews of an older folder are dropped.
    pub generation: u64,
    /// Why the last folder could not be opened. The previous folder stays open.
    pub error: Option<String>,
    /// Set by keyboard moves and by opening a folder so the page list scrolls to the selected row.
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

impl App {
    pub fn new(
        ctx: egui::Context,
        folder: Option<PathBuf>,
    ) -> Result<App, rayon::ThreadPoolBuildError> {
        let mut app = App {
            project: None,
            selected: 0,
            generation: 0,
            error: None,
            reveal_selected: false,
            loader: Loader::spawn(ctx.clone()),
            thumbnails: Thumbnails::new(ctx)?,
            thumbs: Vec::new(),
            preview: None,
            last_requested: None,
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
                for (index, entry) in project.entries.iter().enumerate() {
                    self.thumbnails
                        .request(self.generation, index, entry.path.clone());
                }
                self.thumbs = vec![None; project.entries.len()];
                self.project = Some(project);
                self.selected = 0;
                self.reveal_selected = true;
                self.preview = None;
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
            let Some(image) = thumb.image else {
                continue;
            };
            let texture = ctx.load_texture(
                format!("thumb {}", thumb.index),
                image,
                egui::TextureOptions::LINEAR,
            );
            if let Some(slot) = self.thumbs.get_mut(thumb.index) {
                *slot = Some(texture);
            }
        }
    }

    fn is_current(&self, index: usize, generation: u64) -> bool {
        generation == self.generation && index == self.selected
    }

    /// Asks the worker for the selected page whenever the folder or the selection has changed.
    fn request_preview_if_changed(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let key = (self.generation, self.selected);
        if self.last_requested == Some(key) {
            return;
        }
        self.last_requested = Some(key);
        if let Some(entry) = project.entries.get(self.selected) {
            self.loader.request(PreviewRequest {
                index: self.selected,
                generation: self.generation,
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
        panels::top::show(self, ui);
        panels::status::show(self, ui);
        panels::pages::show(self, ui);
        panels::adjust::show(ui);
        editor::show(self, ui);
    }
}
