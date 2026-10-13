//! The export window: the output options, the export running on a background thread, and its
//! progress and outcome.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use pagecrop_core::export::{ExportOptions, Outcome, Progress, plan, run};
use pagecrop_core::geometry::Resize;

use crate::app::App;

/// How the exported pictures are sized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeMode {
    Keep,
    LongEdge,
    Percent,
}

/// What the export thread sends to the window.
pub enum Msg {
    Progress(Progress),
    Failed(String),
}

/// One export, from start to finish.
pub struct Run {
    pub total: usize,
    pub done: usize,
    pub written: usize,
    pub skipped: usize,
    pub failed: usize,
    /// File name and reason, for each picture that failed.
    pub failures: Vec<(String, String)>,
    pub cancelled: bool,
    pub finished: bool,
    pub error: Option<String>,
    /// File names by page index, to name the pictures that failed.
    names: Vec<String>,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Msg>,
}

pub struct ExportDialog {
    pub open: bool,
    pub out_dir: String,
    pub mode: ResizeMode,
    pub long_edge: u32,
    pub percent: f32,
    pub quality: u8,
    pub overwrite: bool,
    /// A problem with the options, shown above the buttons.
    pub message: Option<String>,
    pub run: Option<Run>,
}

impl Default for ExportDialog {
    fn default() -> Self {
        ExportDialog {
            open: false,
            out_dir: String::new(),
            mode: ResizeMode::LongEdge,
            long_edge: 1600,
            percent: 50.0,
            quality: 90,
            overwrite: false,
            message: None,
            run: None,
        }
    }
}

impl ExportDialog {
    /// Fills the fields in from the options saved with the folder.
    pub fn load_from(&mut self, options: &ExportOptions) {
        self.out_dir = options.out_dir.display().to_string();
        match options.resize {
            Resize::None => self.mode = ResizeMode::Keep,
            Resize::LongEdge(n) => {
                self.mode = ResizeMode::LongEdge;
                self.long_edge = n;
            }
            Resize::Percent(p) => {
                self.mode = ResizeMode::Percent;
                self.percent = p;
            }
        }
        self.quality = options.quality;
        self.overwrite = options.overwrite;
    }

    /// The resize the fields describe.
    pub fn resize(&self) -> Resize {
        match self.mode {
            ResizeMode::Keep => Resize::None,
            ResizeMode::LongEdge => Resize::LongEdge(self.long_edge),
            ResizeMode::Percent => Resize::Percent(self.percent),
        }
    }

    /// The options the fields describe.
    pub fn options(&self) -> ExportOptions {
        ExportOptions {
            out_dir: PathBuf::from(self.out_dir.trim()),
            resize: self.resize(),
            quality: self.quality,
            overwrite: self.overwrite,
        }
    }

    /// Whether an export is running right now.
    pub fn is_running(&self) -> bool {
        self.run.as_ref().is_some_and(|r| !r.finished)
    }
}

impl App {
    /// Starts exporting the open folder with the dialog's options, on a background thread. The
    /// options are saved with the folder first.
    pub fn start_export(&mut self, ctx: &egui::Context) {
        let options = self.export_dialog.options();
        if options.out_dir.as_os_str().is_empty() {
            self.export_dialog.message = Some("Choose an output folder first.".to_owned());
            return;
        }
        let Some(project) = self.project.as_mut() else {
            return;
        };
        project.export = options.clone();
        self.export_dialog.message = None;
        self.save_settings();
        let Some(project) = self.project.clone() else {
            return;
        };
        let names = project
            .entries
            .iter()
            .map(|e| e.file_name.clone())
            .collect();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let thread_ctx = ctx.clone();
        std::thread::spawn(move || {
            let send = |msg: Msg| {
                let _ = tx.send(msg);
                thread_ctx.request_repaint();
            };
            match plan(&project, &options) {
                Err(err) => send(Msg::Failed(err.to_string())),
                Ok(jobs) => {
                    let report = |p: Progress| send(Msg::Progress(p));
                    if let Err(err) = run(&jobs, &options, &thread_cancel, &report) {
                        send(Msg::Failed(err.to_string()));
                    }
                }
            }
        });
        self.export_dialog.run = Some(Run {
            total: 0,
            done: 0,
            written: 0,
            skipped: 0,
            failed: 0,
            failures: Vec::new(),
            cancelled: false,
            finished: false,
            error: None,
            names,
            cancel,
            rx,
        });
    }

    /// Takes the messages the export thread has sent since the last frame.
    pub fn take_export_messages(&mut self) {
        let Some(run) = self.export_dialog.run.as_mut() else {
            return;
        };
        loop {
            let msg = match run.rx.try_recv() {
                Ok(msg) => msg,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !run.finished {
                        run.error = Some("The export stopped unexpectedly.".to_owned());
                        run.finished = true;
                    }
                    break;
                }
            };
            match msg {
                Msg::Failed(err) => {
                    run.error = Some(err);
                    run.finished = true;
                }
                Msg::Progress(Progress::Started { total }) => run.total = total,
                Msg::Progress(Progress::Item { index, outcome }) => {
                    run.done += 1;
                    match outcome {
                        Outcome::Written => run.written += 1,
                        Outcome::Skipped => run.skipped += 1,
                        Outcome::Failed(reason) => {
                            run.failed += 1;
                            let name = run.names.get(index).cloned().unwrap_or_default();
                            run.failures.push((name, reason));
                        }
                    }
                }
                Msg::Progress(Progress::Finished { cancelled, .. }) => {
                    run.cancelled = cancelled;
                    run.finished = true;
                }
            }
        }
    }
}

/// Draws the export window while it is open. It stays open while an export runs, because the rest
/// of the window is disabled until then and Cancel is in here.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let running = app.export_dialog.is_running();
    let mut open = app.export_dialog.open || running;
    egui::Window::new("Export")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_enabled_ui(!running, |ui| options(app, ui));
            ui.separator();
            progress(app, ui);
            buttons(app, ui);
        });
    app.export_dialog.open = open || app.export_dialog.is_running();
}

fn options(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Output folder");
        ui.text_edit_singleline(&mut app.export_dialog.out_dir);
        if ui.button("Choose…").clicked()
            && let Some(dir) = rfd::FileDialog::new().pick_folder()
        {
            app.export_dialog.out_dir = dir.display().to_string();
        }
    });
    let dialog = &mut app.export_dialog;
    ui.horizontal(|ui| {
        ui.radio_value(&mut dialog.mode, ResizeMode::Keep, "Keep size");
        ui.radio_value(&mut dialog.mode, ResizeMode::LongEdge, "Long edge");
        ui.add_enabled(
            dialog.mode == ResizeMode::LongEdge,
            egui::DragValue::new(&mut dialog.long_edge)
                .range(1..=10_000)
                .suffix(" px"),
        );
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut dialog.mode, ResizeMode::Percent, "Percent");
        ui.add_enabled(
            dialog.mode == ResizeMode::Percent,
            egui::DragValue::new(&mut dialog.percent)
                .range(1.0..=100.0)
                .suffix(" %"),
        );
    });
    ui.add(egui::Slider::new(&mut dialog.quality, 60..=100).text("Quality"));
    ui.checkbox(&mut dialog.overwrite, "Replace existing files");
    let existing = app.project.as_ref().map_or(0, |project| {
        let out = PathBuf::from(app.export_dialog.out_dir.trim());
        project
            .entries
            .iter()
            .filter(|e| out.join(&e.file_name).exists())
            .count()
    });
    let pages = app.page_count();
    ui.label(format!(
        "{pages} pages, {existing} already exist in the output folder"
    ));
}

fn progress(app: &App, ui: &mut egui::Ui) {
    let Some(run) = app.export_dialog.run.as_ref() else {
        return;
    };
    if let Some(error) = &run.error {
        ui.colored_label(egui::Color32::RED, error);
    }
    if run.finished {
        ui.label(format!(
            "{} written, {} skipped, {} failed",
            run.written, run.skipped, run.failed
        ));
        if run.cancelled {
            ui.label("Cancelled.");
        }
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for (name, reason) in &run.failures {
                    ui.colored_label(egui::Color32::RED, format!("{name}: {reason}"));
                }
            });
    } else {
        let fraction = if run.total == 0 {
            0.0
        } else {
            run.done as f32 / run.total as f32
        };
        ui.add(egui::ProgressBar::new(fraction).text(format!("{} / {}", run.done, run.total)));
    }
}

fn buttons(app: &mut App, ui: &mut egui::Ui) {
    if let Some(message) = &app.export_dialog.message {
        ui.colored_label(egui::Color32::RED, message);
    }
    ui.horizontal(|ui| {
        if app.export_dialog.is_running() {
            if ui.button("Cancel").clicked()
                && let Some(run) = app.export_dialog.run.as_ref()
            {
                run.cancel.store(true, Ordering::Relaxed);
            }
        } else if ui.button("Start").clicked() {
            app.start_export(ui.ctx());
        }
    });
}
