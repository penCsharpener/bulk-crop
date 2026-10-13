//! Bottom status bar: the page position and name, and any error from opening a folder.

use crate::app::App;

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
        Some(entry) => format!(
            "{} / {} · {}",
            app.selected + 1,
            project.entries.len(),
            entry.file_name
        ),
        None => format!("0 / {}", project.entries.len()),
    }
}
