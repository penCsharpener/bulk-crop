//! Left panel listing the pages.

pub fn show(ui: &mut egui::Ui) {
    egui::Panel::left("pages").exact_size(220.0).show(ui, |ui| {
        ui.heading("Pages");
    });
}
