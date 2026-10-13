//! Bottom status bar: the page position, name and full size, and any error from opening a folder.

use crate::app::{App, Content};

pub fn show(app: &App, ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(status_text(app));
            if let Some(error) = &app.error {
                ui.colored_label(egui::Color32::RED, error);
            }
        });
    });
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
