//! Bottom status bar: the page position, name, full size and crop frame size, any error from
//! opening a folder, and the outcome of saving.

use pagecrop_core::geometry::{Size, base_size, output_size};

use crate::app::{App, Content};

pub fn show(app: &App, ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(status_text(app));
            if let Some(error) = &app.error {
                ui.colored_label(egui::Color32::RED, error);
            }
            if let Some(notice) = &app.notice {
                ui.label(notice);
            }
            save_status(app, ui);
        });
    });
}

/// How long "Saved" stays in the status bar.
const SAVED_FOR: std::time::Duration = std::time::Duration::from_secs(2);

/// "Saved" for a moment after a save, or the reason the last save failed.
fn save_status(app: &App, ui: &mut egui::Ui) {
    if let Some(error) = &app.save_error {
        ui.colored_label(
            egui::Color32::from_rgb(255, 165, 0),
            format!("Could not save: {error}"),
        );
    } else if let Some(at) = app.saved_at {
        let age = at.elapsed();
        if age < SAVED_FOR {
            ui.label("Saved");
            ui.ctx().request_repaint_after(SAVED_FOR - age);
        }
    }
}

fn status_text(app: &App) -> String {
    let Some(project) = &app.project else {
        return "No folder open".to_owned();
    };
    match project.entries.get(app.selected) {
        Some(entry) => {
            let mut text = format!(
                "{} / {} · {}",
                app.selected + 1,
                project.entries.len(),
                entry.file_name
            );
            if let Some((w, h)) = full_size_of_selected(app) {
                text.push_str(&format!(" · {w}×{h}"));
                text.push_str(&frame_text(app, (w, h)));
            }
            text
        }
        None => format!("0 / {}", project.entries.len()),
    }
}

/// Size of the selected page's whole picture, once its preview has loaded.
fn full_size_of_selected(app: &App) -> Option<(u32, u32)> {
    match app.selected_content() {
        Some(Content::Picture { full_size, .. }) => Some(*full_size),
        _ => None,
    }
}

/// The crop frame's size in source pixels, and the size the export will give it.
fn frame_text(app: &App, full_size: (u32, u32)) -> String {
    let adjust = app.effective();
    let base = base_size(
        Size {
            w: full_size.0 as f32,
            h: full_size.1 as f32,
        },
        adjust.quarter_turns,
    );
    let w = (adjust.crop.w * base.w).round().max(1.0) as u32;
    let h = (adjust.crop.h * base.h).round().max(1.0) as u32;
    let (out_w, out_h) = output_size(base, adjust.crop, app.export_dialog.resize());
    format!(" · frame {w}×{h} → {out_w}×{out_h}")
}
