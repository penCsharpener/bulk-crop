//! Top bar with the application name and the open-folder button.

use crate::app::App;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("pagecrop");
            if ui.button("Open folder…").clicked() {
                open_with_dialog(app);
            }
        });
    });
}

fn open_with_dialog(app: &mut App) {
    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
        app.open_folder(&dir);
    }
}
