//! Top bar with the application name.

pub fn show(ui: &mut egui::Ui) {
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.heading("pagecrop");
    });
}
