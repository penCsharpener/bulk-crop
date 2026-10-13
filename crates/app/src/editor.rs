//! Central area where the current page will be edited.

pub fn show(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.centered_and_justified(|ui| {
            ui.label("Open a folder to begin");
        });
    });
}
