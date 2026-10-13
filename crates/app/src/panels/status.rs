//! Bottom status bar.

pub fn show(ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.label("No folder open");
    });
}
