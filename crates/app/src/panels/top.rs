//! Top bar: the name, the folder and export buttons, undo and redo, and the sort switch.

use pagecrop_core::model::SortBy;

use crate::app::App;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("pagecrop");
            if ui.button("Open folder…").clicked() {
                open_with_dialog(app);
            }
            if ui.button("Export…").clicked() {
                app.export_dialog.open = true;
            }
            ui.separator();
            if ui
                .add_enabled(app.history.can_undo(), egui::Button::new("↶"))
                .clicked()
            {
                app.undo();
            }
            if ui
                .add_enabled(app.history.can_redo(), egui::Button::new("↷"))
                .clicked()
            {
                app.redo();
            }
            ui.separator();
            let current = app.project.as_ref().map(|p| p.sort);
            for (label, by) in [("Name", SortBy::Name), ("Date", SortBy::Date)] {
                if ui.selectable_label(current == Some(by), label).clicked() {
                    app.request_sort(by);
                }
            }
        });
    });
}

fn open_with_dialog(app: &mut App) {
    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
        app.open_folder(&dir);
    }
}
