//! Right panel for the adjustments of the current page.

pub fn show(ui: &mut egui::Ui) {
    egui::Panel::right("adjust")
        .exact_size(260.0)
        .show(ui, |ui| {
            ui.heading("Adjust");
        });
}
