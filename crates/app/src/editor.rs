//! Central area: the selected page drawn to fit, or the message that stands in for it.

use crate::app::{App, Content};

pub fn show(app: &App, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        if app.project.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Open a folder to begin");
            });
            return;
        }
        if app.page_count() == 0 {
            ui.centered_and_justified(|ui| {
                ui.label("No photos in this folder");
            });
            return;
        }
        match app.selected_content() {
            Some(Content::Picture { texture, .. }) => draw_fitted(ui, texture),
            Some(Content::Failed(message)) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::RED, message);
                });
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Loading…");
                });
            }
        }
    });
}

/// Draws the texture centred in the area, scaled to fit with a 4 % margin.
fn draw_fitted(ui: &mut egui::Ui, texture: &egui::TextureHandle) {
    let area = ui.available_rect_before_wrap();
    paint_fitted(ui.painter(), texture, area.shrink2(area.size() * 0.04));
}

/// Paints the texture centred in `area`, scaled to fit without changing its shape.
pub fn paint_fitted(painter: &egui::Painter, texture: &egui::TextureHandle, area: egui::Rect) {
    let [w, h] = texture.size().map(|v| v as f32);
    if w < 1.0 || h < 1.0 {
        return;
    }
    let scale = (area.width() / w).min(area.height() / h);
    let rect = egui::Rect::from_center_size(area.center(), egui::vec2(w, h) * scale);
    let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), rect, full_uv, egui::Color32::WHITE);
}
