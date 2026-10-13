//! Central area: the selected page drawn to fit, or the message that stands in for it.

use pagecrop_core::model::Adjust;

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
            Some(Content::Picture { texture, .. }) => draw_turned(ui, texture, app.effective()),
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

/// Draws the picture turned by its settings, its base size fitted into the area with a 4 % margin.
/// The base is the picture after its quarter turns; the fine angle turns it further.
fn draw_turned(ui: &mut egui::Ui, texture: &egui::TextureHandle, adjust: Adjust) {
    let area = ui.available_rect_before_wrap();
    let inner = area.shrink2(area.size() * 0.04);
    let [w, h] = texture.size().map(|v| v as f32);
    let (base_w, base_h) = if adjust.quarter_turns % 2 == 1 {
        (h, w)
    } else {
        (w, h)
    };
    if base_w < 1.0 || base_h < 1.0 {
        return;
    }
    let scale = (inner.width() / base_w).min(inner.height() / base_h);
    let centre = inner.center();
    let rect = egui::Rect::from_center_size(centre, egui::vec2(w, h) * scale);
    let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let mut mesh = egui::Mesh::with_texture(texture.id());
    mesh.add_rect_with_uv(rect, full_uv, egui::Color32::WHITE);
    let degrees = f32::from(adjust.quarter_turns) * 90.0 + adjust.angle_deg;
    mesh.rotate(egui::emath::Rot2::from_angle(degrees.to_radians()), centre);
    ui.painter().add(egui::Shape::mesh(mesh));
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
