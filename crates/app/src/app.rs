//! The application state and the frame that lays out the panels.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbImage;
use pagecrop_core::crop_edit::Handle;
use pagecrop_core::error::CoreError;
use pagecrop_core::geometry::{Size, base_size, fit_inside, quarter_turn};
use pagecrop_core::history::{Edit, History};
use pagecrop_core::model::{Adjust, CropRect, Project, SortBy, read_ahead_order};
use pagecrop_core::persist::{self, SIDECAR_NAME};

use crate::export_dialog::{self, ExportDialog};
use crate::loader::{Loader, PreviewReply, PreviewRequest, Thumbnails};
use crate::{editor, help, panels};

/// All state of the window.
pub struct App {
    /// The open folder, if any.
    pub project: Option<Project>,
    /// Index of the page on screen. 0 when the folder is empty.
    pub selected: usize,
    /// Goes up each time a folder opens. Thumbnails of an older folder are dropped.
    pub generation: u64,
    /// Like `generation`, and it also goes up on a sort, because that moves pages to new indices.
    pub preview_generation: u64,
    /// Why the last folder could not be opened. The previous folder stays open.
    pub error: Option<String>,
    /// Set by keyboard moves, by opening a folder and by a sort so the page list scrolls to the selected row.
    pub reveal_selected: bool,
    loader: Loader,
    thumbnails: Thumbnails,
    /// One thumbnail per page of the open folder; None until it arrives, or for good if the picture
    /// could not be decoded.
    pub thumbs: Vec<Option<egui::TextureHandle>>,
    /// The previews in memory: the selected page and the pages read ahead of it.
    pub previews: PreviewCache,
    /// The (generation, selected) pair a preview was last requested for.
    last_requested: Option<(u64, usize)>,
    /// A sort waiting for the user to confirm it, because pages have their own settings.
    pending_sort: Option<SortBy>,
    /// The crop-frame drag in progress, if any.
    pub drag: Option<Drag>,
    /// Whether a grid is drawn over the crop frame.
    pub show_grid: bool,
    /// Whether the selected page is shown as it will be exported, instead of with its frame.
    pub show_result: bool,
    /// The last rendered result view.
    pub result: Option<ResultView>,
    /// The picture with the tone applied, when the tone is not the default.
    pub tone: Option<ToneView>,
    /// When the settings were last saved, while the save succeeded.
    pub saved_at: Option<Instant>,
    /// Why the last save failed; cleared by the next save that works.
    pub save_error: Option<String>,
    /// A note about the folder, such as an unreadable settings file that was kept.
    pub notice: Option<String>,
    pub export_dialog: ExportDialog,
    /// Whether the window that lists the shortcuts is open.
    pub help_open: bool,
    /// The changes that can be undone and redone.
    pub history: History,
    /// How far the picture is zoomed in the editor: 1 fits it to the area, up to 8.
    pub zoom_factor: f32,
    /// Where the centre of the picture sits, in points away from the centre of the fitted area.
    pub pan: egui::Vec2,
    /// The page being changed and its settings before the change, from `begin_edit` until
    /// `commit_edit`.
    edit_start: Option<(usize, Option<Adjust>)>,
    /// Set just before an edit that continues the last one, so the two undo together.
    merge_next: bool,
    /// The page whose fine angle the last key press changed, while no other edit has come since.
    nudge_page: Option<usize>,
    /// The frame of the selected page when the current angle drag began, so dragging the angle out
    /// and back gives the frame back. None while no angle drag is running.
    pub angle_start: Option<CropRect>,
    /// How close the pointer must be to a frame edge or corner to grab it, in per cent of the
    /// picture's shorter side. An app-wide setting, kept between runs.
    pub handle_reach_percent: f32,
}

/// The range of `handle_reach_percent`, in per cent.
pub const REACH_RANGE: (f32, f32) = (1.0, 15.0);

/// The key under which the handle reach is stored in eframe's settings.
const REACH_KEY: &str = "handle_reach_percent";

/// How many previews are kept in memory.
const CACHE_SIZE: usize = 12;

/// A preview of one page.
pub struct Preview {
    pub index: usize,
    pub content: Content,
}

/// The previews in memory, least recently used first.
#[derive(Default)]
pub struct PreviewCache {
    entries: Vec<Preview>,
}

impl PreviewCache {
    pub fn get(&self, index: usize) -> Option<&Preview> {
        self.entries.iter().find(|p| p.index == index)
    }

    pub fn contains(&self, index: usize) -> bool {
        self.get(index).is_some()
    }

    /// Marks page `index` as the most recently used. Does nothing when it is not cached.
    pub fn use_page(&mut self, index: usize) {
        if let Some(pos) = self.entries.iter().position(|p| p.index == index) {
            let preview = self.entries.remove(pos);
            self.entries.push(preview);
        }
    }

    /// Stores a preview as the most recently used one, and drops the least recently used when the
    /// cache is full.
    pub fn insert(&mut self, preview: Preview) {
        self.entries.retain(|p| p.index != preview.index);
        self.entries.push(preview);
        if self.entries.len() > CACHE_SIZE {
            self.entries.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub enum Content {
    Picture {
        texture: egui::TextureHandle,
        /// The preview as pixels, for rendering the result view.
        rgb: RgbImage,
        /// Size of the whole picture before shrinking, in pixels.
        full_size: (u32, u32),
    },
    Failed(String),
}

/// The picture with the tone applied. Kept until the page or the tone values change.
pub struct ToneView {
    /// The selected page, its preview generation, brightness, contrast and greyscale.
    pub key: (usize, u64, i32, i32, bool),
    pub texture: egui::TextureHandle,
}

/// The rendered result of a page. Kept until the page, its settings or the view size change.
pub struct ResultView {
    /// The selected page, the preview generation, its settings and the view's longer side in points.
    pub key: (usize, u64, Adjust, u32),
    pub texture: egui::TextureHandle,
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
        storage: Option<&dyn eframe::Storage>,
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
            previews: PreviewCache::default(),
            last_requested: None,
            pending_sort: None,
            drag: None,
            show_grid: false,
            show_result: false,
            result: None,
            tone: None,
            saved_at: None,
            save_error: None,
            notice: None,
            export_dialog: ExportDialog::default(),
            help_open: false,
            history: History::default(),
            zoom_factor: 1.0,
            pan: egui::Vec2::ZERO,
            edit_start: None,
            merge_next: false,
            nudge_page: None,
            angle_start: None,
            handle_reach_percent: 5.0,
        };
        if let Some(storage) = storage
            && let Some(p) = eframe::get_value::<f32>(storage, REACH_KEY)
        {
            app.handle_reach_percent = p.clamp(REACH_RANGE.0, REACH_RANGE.1);
        }
        if let Some(dir) = folder {
            app.open_folder(&dir);
        }
        Ok(app)
    }

    pub fn open_folder(&mut self, dir: &Path) {
        let (opened, notice) = match Project::open(dir) {
            Ok(project) => (Ok(project), None),
            Err(CoreError::Sidecar { path, .. }) => (
                Project::open_without_settings(dir),
                Some(keep_unreadable_settings(dir, &path)),
            ),
            Err(err) => (Err(err), None),
        };
        match opened {
            Ok(project) => {
                self.notice = notice;
                self.install(project);
            }
            Err(err) => self.error = Some(err.to_string()),
        }
    }

    /// Makes `project` the open folder: its thumbnails are requested and the selection reset.
    fn install(&mut self, project: Project) {
        self.export_dialog.load_from(&project.export);
        self.generation += 1;
        self.preview_generation += 1;
        for entry in &project.entries {
            self.thumbnails
                .request(self.generation, entry.file_name.clone(), entry.path.clone());
        }
        self.thumbs = vec![None; project.entries.len()];
        self.project = Some(project);
        self.selected = 0;
        self.reveal_selected = true;
        self.previews.clear();
        self.pending_sort = None;
        self.history.clear();
        self.edit_start = None;
        self.error = None;
        self.saved_at = None;
        self.save_error = None;
    }

    /// Writes the settings of the open folder. The outcome shows in the status bar.
    pub(crate) fn save_settings(&mut self) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        match persist::save(project) {
            Ok(()) => {
                self.saved_at = Some(Instant::now());
                self.save_error = None;
            }
            Err(err) => {
                self.save_error = Some(err.to_string());
                self.saved_at = None;
            }
        }
    }

    pub fn page_count(&self) -> usize {
        self.project.as_ref().map_or(0, |p| p.entries.len())
    }

    /// Selects page `index`, clamped to the last page.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.page_count().saturating_sub(1));
        self.nudge_page = None;
    }

    /// Takes the finished previews of the open folder into the cache.
    fn take_replies(&mut self, ctx: &egui::Context) {
        while let Some(reply) = self.loader.poll() {
            match reply {
                PreviewReply::Loaded {
                    index,
                    generation,
                    image,
                    rgb,
                    full_size,
                } if generation == self.preview_generation => {
                    let texture = ctx.load_texture("preview", image, egui::TextureOptions::LINEAR);
                    self.previews.insert(Preview {
                        index,
                        content: Content::Picture {
                            texture,
                            rgb,
                            full_size,
                        },
                    });
                }
                PreviewReply::Failed {
                    index,
                    generation,
                    message,
                } if generation == self.preview_generation => {
                    self.previews.insert(Preview {
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
        self.previews.get(self.selected).map(|p| &p.content)
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

    /// The size of the selected page's picture after its quarter turns, in full-size pixels. None
    /// while the preview is loading or failed.
    pub fn selected_base(&self) -> Option<Size> {
        let Some(Content::Picture { full_size, .. }) = self.selected_content() else {
            return None;
        };
        let src = Size {
            w: full_size.0 as f32,
            h: full_size.1 as f32,
        };
        Some(base_size(src, self.effective().quarter_turns))
    }

    /// The settings of the selected page with the fine angle `angle_deg`. The frame is fitted inside
    /// the picture turned by that angle, starting from the frame as it was when the angle drag began.
    pub fn angled(&self, angle_deg: f32) -> Adjust {
        let current = self.effective();
        let crop = match self.selected_base() {
            Some(base) => fit_inside(self.angle_start.unwrap_or(current.crop), base, angle_deg),
            None => current.crop,
        };
        Adjust {
            angle_deg,
            crop,
            ..current
        }
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
        let base = if self.page_has_key() {
            "Own settings".to_owned()
        } else {
            match project.key_index(self.selected) {
                Some(k) => format!("Follows page {}", k + 1),
                None => "Default settings".to_owned(),
            }
        };
        match self.side_label() {
            Some(side) => format!("{base} · {side}"),
            None => base,
        }
    }

    /// "side A" or "side B" when the selected page alternates, else None.
    pub fn side_label(&self) -> Option<&'static str> {
        let side = self.project.as_ref()?.side(self.selected)?;
        Some(if side == 0 { "side A" } else { "side B" })
    }

    /// Starts an edit: remembers the settings of the selected page as they are before the change.
    pub fn begin_edit(&mut self) {
        self.edit_start = self
            .project
            .as_ref()
            .and_then(|p| p.entries.get(self.selected))
            .map(|entry| (self.selected, entry.key));
    }

    /// Whether an edit has begun and not yet been committed, such as while a slider is dragged.
    pub fn editing(&self) -> bool {
        self.edit_start.is_some()
    }

    /// Finishes an edit: records it in the history when the page changed, and saves the settings.
    pub fn commit_edit(&mut self) {
        let merge = std::mem::take(&mut self.merge_next);
        self.nudge_page = None;
        if let Some((index, before)) = self.edit_start.take()
            && let Some(entry) = self.project.as_ref().and_then(|p| p.entries.get(index))
        {
            let edit = Edit {
                index,
                before,
                after: entry.key,
            };
            if merge {
                self.history.extend(edit);
            } else {
                self.history.push(edit);
            }
        }
        self.save_settings();
    }

    /// Returns the editor to the fitted view.
    pub fn reset_view(&mut self) {
        self.zoom_factor = 1.0;
        self.pan = egui::Vec2::ZERO;
    }

    /// Takes back the last change and selects the page it changed.
    pub fn undo(&mut self) {
        self.step_history(History::undo);
    }

    /// Applies the last change that was taken back and selects the page it changed.
    pub fn redo(&mut self) {
        self.step_history(History::redo);
    }

    fn step_history(&mut self, step: fn(&mut History, &mut Project) -> Option<usize>) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let Some(index) = step(&mut self.history, project) else {
            return;
        };
        self.select(index);
        self.reveal_selected = true;
        self.save_settings();
    }

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
        // The history names pages by index, and a sort moves them, so it is forgotten.
        self.history.clear();
        self.edit_start = None;
        // Previews still on their way were asked for by old index, so they are dropped.
        self.preview_generation += 1;
        self.previews.clear();
        self.last_requested = None;
        self.save_settings();
    }

    /// Asks for a folder in the file dialog and opens it.
    pub fn choose_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            self.open_folder(&dir);
        }
    }

    /// Makes one whole edit of the selected page: begin, apply, commit.
    pub fn edit_once(&mut self, adjust: Adjust) {
        self.begin_edit();
        self.apply_edit(adjust);
        self.commit_edit();
    }

    /// Moves the fine angle of the selected page by `degrees`, kept to the hundredth and within the
    /// slider's range.
    pub fn nudge_angle(&mut self, degrees: f32) {
        let page = self.selected;
        let angle = ((self.effective().angle_deg + degrees) * 100.0).round() / 100.0;
        // Presses in a row on one page are one undo step.
        self.merge_next = self.nudge_page == Some(page);
        self.edit_once(self.angled(angle.clamp(-15.0, 15.0)));
        self.nudge_page = Some(page);
    }

    /// Turns the selected page a quarter turn clockwise or anticlockwise.
    pub fn turn(&mut self, clockwise: bool) {
        let turned = quarter_turn(self.effective(), clockwise);
        self.edit_once(turned);
    }

    /// Selects the next or previous page that has its own settings. The selection stays when there
    /// is none.
    pub fn select_keyed(&mut self, forward: bool) {
        let Some(index) = self
            .project
            .as_ref()
            .and_then(|p| p.keyed_page_from(self.selected, forward))
        else {
            return;
        };
        self.select(index);
        self.reveal_selected = true;
    }

    /// Asks the worker for previews whenever the folder or the selection changes: the selected page
    /// first, then the pages read ahead of it that are not cached yet.
    fn request_preview_if_changed(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let key = (self.preview_generation, self.selected);
        if self.last_requested == Some(key) {
            return;
        }
        self.last_requested = Some(key);
        self.previews.use_page(self.selected);
        let generation = self.preview_generation;
        let wanted = read_ahead_order(self.selected, project.entries.len())
            .into_iter()
            .filter(|&index| !self.previews.contains(index))
            .filter_map(|index| {
                let entry = project.entries.get(index)?;
                Some(PreviewRequest {
                    index,
                    generation,
                    path: entry.path.clone(),
                })
            })
            .collect();
        self.loader.request(wanted);
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        // Letters, digits and arrows typed into a text field belong to the field, not to the
        // shortcuts.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = |key: egui::Key| ctx.input(|input| input.key_pressed(key));
        let modifiers = ctx.input(|input| input.modifiers);
        let plain = !modifiers.command && !modifiers.alt;
        let idle = !self.export_dialog.is_running() && !self.editing();
        if modifiers.command && pressed(egui::Key::E) {
            self.export_dialog.open = !self.export_dialog.open;
        }
        if modifiers.command && idle && pressed(egui::Key::O) {
            self.choose_folder();
        }
        if pressed(egui::Key::F1) {
            self.help_open = !self.help_open;
        }
        if pressed(egui::Key::Num0) {
            self.reset_view();
        }
        let (undo, redo) = ctx.input(history_keys_pressed);
        if undo && idle {
            self.undo();
        }
        if redo && idle {
            self.redo();
        }
        if ctx.input(result_key_pressed) {
            self.show_result = !self.show_result;
        }
        if plain && pressed(egui::Key::G) {
            self.show_grid = !self.show_grid;
        }
        if self.page_count() == 0 {
            return;
        }
        if plain && idle {
            self.edit_keys(&pressed, modifiers.shift);
        }
        if plain && pressed(egui::Key::N) {
            self.select_keyed(!modifiers.shift);
        }
        let current = self.selected;
        let last = self.page_count().saturating_sub(1);
        let target = ctx.input(|input| key_target(input, current, last));
        if let Some(index) = target {
            self.select(index);
            self.reveal_selected = true;
        }
    }

    /// The keys that change the settings of the selected page: R turns it, [ and ] move the fine
    /// angle (Shift for whole degrees), and Delete resets the page.
    fn edit_keys(&mut self, pressed: &dyn Fn(egui::Key) -> bool, shift: bool) {
        if pressed(egui::Key::R) {
            self.turn(!shift);
        }
        // With Shift held, the brackets arrive as the curly-bracket keys.
        let step = if shift { 1.0 } else { 0.1 };
        if pressed(egui::Key::OpenBracket) || pressed(egui::Key::OpenCurlyBracket) {
            self.nudge_angle(-step);
        }
        if pressed(egui::Key::CloseBracket) || pressed(egui::Key::CloseCurlyBracket) {
            self.nudge_angle(step);
        }
        if pressed(egui::Key::Delete) && self.page_has_key() {
            self.reset_page();
        }
    }
}

/// Whether undo and redo were pressed this frame: Ctrl+Z undoes, Ctrl+Shift+Z and Ctrl+Y redo.
fn history_keys_pressed(input: &egui::InputState) -> (bool, bool) {
    let command = input.modifiers.command;
    let shift = input.modifiers.shift;
    let undo = command && !shift && input.key_pressed(egui::Key::Z);
    let redo =
        command && ((shift && input.key_pressed(egui::Key::Z)) || input.key_pressed(egui::Key::Y));
    (undo, redo)
}

/// Whether P went down this frame on its own. Key repeat and P with a modifier do not count.
fn result_key_pressed(input: &egui::InputState) -> bool {
    input.events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key {
                key: egui::Key::P,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } if modifiers.is_none()
        )
    })
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
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, REACH_KEY, &self.handle_reach_percent);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_keys(ui.ctx());
        self.take_replies(ui.ctx());
        self.take_thumbs(ui.ctx());
        self.request_preview_if_changed();
        self.confirm_sort(ui.ctx());
        self.take_export_messages();
        let running = self.export_dialog.is_running();
        ui.add_enabled_ui(!running, |ui| {
            panels::top::show(self, ui);
            panels::status::show(self, ui);
            panels::pages::show(self, ui);
            panels::adjust::show(self, ui);
            editor::show(self, ui);
        });
        export_dialog::show(self, ui.ctx());
        help::show(self, ui.ctx());
    }
}

/// Copies an unreadable settings file to `pagecrop.json.bak` and says so in a notice.
fn keep_unreadable_settings(dir: &Path, path: &Path) -> String {
    let backup = dir.join(format!("{SIDECAR_NAME}.bak"));
    match std::fs::copy(path, &backup) {
        Ok(_) => "Settings file was unreadable, kept as pagecrop.json.bak".to_owned(),
        Err(err) => format!("Settings file was unreadable and could not be kept: {err}"),
    }
}
