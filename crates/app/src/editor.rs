//! Central area: the selected page turned by its settings, with its crop frame on top.

use pagecrop_core::crop_edit::{Handle, drag as drag_frame, hit_test};
use pagecrop_core::geometry::{Size, base_size, half_extents};
use pagecrop_core::model::{Adjust, CropRect};

use crate::app::{App, Content, Drag};

/// The smallest frame edge, as a fraction of the base picture.
const MIN_FRAME: (f32, f32) = (0.02, 0.02);
/// How close the pointer must be to a handle, in screen points.
const HANDLE_REACH: f32 = 8.0;
const HANDLE_SIZE: f32 = 9.0;
const ACCENT: egui::Color32 = egui::Color32::from_rgb(64, 156, 255);

pub fn show(app: &mut App, ui: &mut egui::Ui) {
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
        let texture = match app.selected_content() {
            Some(Content::Picture { texture, .. }) => texture.clone(),
            Some(Content::Failed(message)) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::RED, message);
                });
                return;
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Loading…");
                });
                return;
            }
        };
        canvas(app, ui, &texture);
    });
}

/// The picture with its crop frame: drawing, the cursor and dragging of the frame.
fn canvas(app: &mut App, ui: &mut egui::Ui, texture: &egui::TextureHandle) {
    let (area, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    let adjust = app.effective();
    let Some(layout) = Layout::new(texture, adjust, area) else {
        return;
    };
    let painter = ui.painter_at(area);
    paint_picture(&painter, texture, adjust, &layout);
    paint_frame(&painter, &layout, area, adjust.crop, app.show_grid);
    handle_pointer(app, ui, &response, &layout, adjust);
}

/// Where the picture sits on screen, and how frame fractions map to screen points.
struct Layout {
    centre: egui::Pos2,
    /// Screen points per preview pixel.
    scale: f32,
    /// The preview after its quarter turns, in preview pixels.
    base: Size,
    /// The preview texture's size, in preview pixels.
    texture: egui::Vec2,
}

impl Layout {
    /// Fits the base picture into the area with a 4 % margin. None when there is nothing to draw.
    fn new(texture: &egui::TextureHandle, adjust: Adjust, area: egui::Rect) -> Option<Layout> {
        let [w, h] = texture.size().map(|v| v as f32);
        let base = base_size(Size { w, h }, adjust.quarter_turns);
        let inner = area.shrink2(area.size() * 0.04);
        let scale = (inner.width() / base.w).min(inner.height() / base.h);
        if base.w < 1.0 || base.h < 1.0 || scale.is_nan() || scale <= 0.0 {
            return None;
        }
        Some(Layout {
            centre: inner.center(),
            scale,
            base,
            texture: egui::vec2(w, h),
        })
    }

    /// The base picture's size on screen, in points.
    fn base_points(&self) -> egui::Vec2 {
        egui::vec2(self.base.w, self.base.h) * self.scale
    }

    fn to_frame(&self, pos: egui::Pos2) -> (f32, f32) {
        let b = self.base_points();
        ((pos.x - self.centre.x) / b.x, (pos.y - self.centre.y) / b.y)
    }

    fn to_screen(&self, (x, y): (f32, f32)) -> egui::Pos2 {
        let b = self.base_points();
        egui::pos2(self.centre.x + x * b.x, self.centre.y + y * b.y)
    }

    /// The crop frame as an upright rectangle on screen.
    fn frame_rect(&self, crop: CropRect) -> egui::Rect {
        egui::Rect::from_min_max(
            self.to_screen((crop.cx - crop.w / 2.0, crop.cy - crop.h / 2.0)),
            self.to_screen((crop.cx + crop.w / 2.0, crop.cy + crop.h / 2.0)),
        )
    }
}

/// Draws the picture turned by its settings. The fine angle turns it further.
fn paint_picture(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    adjust: Adjust,
    layout: &Layout,
) {
    let rect = egui::Rect::from_center_size(layout.centre, layout.texture * layout.scale);
    let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let mut mesh = egui::Mesh::with_texture(texture.id());
    mesh.add_rect_with_uv(rect, full_uv, egui::Color32::WHITE);
    let degrees = f32::from(adjust.quarter_turns) * 90.0 + adjust.angle_deg;
    mesh.rotate(
        egui::emath::Rot2::from_angle(degrees.to_radians()),
        layout.centre,
    );
    painter.add(egui::Shape::mesh(mesh));
}

/// Shades what lies outside the frame and draws the frame, its handles and the optional grid.
fn paint_frame(
    painter: &egui::Painter,
    layout: &Layout,
    area: egui::Rect,
    crop: CropRect,
    grid: bool,
) {
    let frame = layout.frame_rect(crop);
    let shade = egui::Color32::from_black_alpha(128);
    let strips = [
        egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, frame.min.y)),
        egui::Rect::from_min_max(egui::pos2(area.min.x, frame.max.y), area.max),
        egui::Rect::from_min_max(
            egui::pos2(area.min.x, frame.min.y),
            egui::pos2(frame.min.x, frame.max.y),
        ),
        egui::Rect::from_min_max(
            egui::pos2(frame.max.x, frame.min.y),
            egui::pos2(area.max.x, frame.max.y),
        ),
    ];
    for strip in strips {
        painter.rect_filled(strip, 0.0, shade);
    }
    if grid {
        paint_grid(painter, frame);
    }
    painter.rect_stroke(
        frame,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::BLACK),
        egui::StrokeKind::Outside,
    );
    painter.rect_stroke(
        frame,
        0.0,
        egui::Stroke::new(1.5, egui::Color32::WHITE),
        egui::StrokeKind::Middle,
    );
    for point in handle_points(frame) {
        let handle = egui::Rect::from_center_size(point, egui::vec2(HANDLE_SIZE, HANDLE_SIZE));
        painter.rect_filled(handle, 1.0, ACCENT);
    }
}

/// Seven evenly spaced lines each way inside the frame, as a reference for level text.
fn paint_grid(painter: &egui::Painter, frame: egui::Rect) {
    let stroke = egui::Stroke::new(0.5, egui::Color32::from_white_alpha(140));
    for i in 1..=7 {
        let t = i as f32 / 8.0;
        let x = frame.min.x + frame.width() * t;
        let y = frame.min.y + frame.height() * t;
        painter.line_segment(
            [egui::pos2(x, frame.min.y), egui::pos2(x, frame.max.y)],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(frame.min.x, y), egui::pos2(frame.max.x, y)],
            stroke,
        );
    }
}

/// The eight handles: corners first, then the middle of each edge.
fn handle_points(frame: egui::Rect) -> [egui::Pos2; 8] {
    let (l, r) = (frame.min.x, frame.max.x);
    let (t, b) = (frame.min.y, frame.max.y);
    let (cx, cy) = (frame.center().x, frame.center().y);
    [
        egui::pos2(l, t),
        egui::pos2(r, t),
        egui::pos2(l, b),
        egui::pos2(r, b),
        egui::pos2(cx, t),
        egui::pos2(cx, b),
        egui::pos2(l, cy),
        egui::pos2(r, cy),
    ]
}

/// The cursor for the part of the frame under the pointer, or while dragging it.
fn cursor_for(handle: Handle) -> egui::CursorIcon {
    match handle {
        Handle::Move => egui::CursorIcon::Move,
        Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
        Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
        Handle::NE | Handle::SW => egui::CursorIcon::ResizeNeSw,
        Handle::NW | Handle::SE => egui::CursorIcon::ResizeNwSe,
    }
}

fn handle_pointer(
    app: &mut App,
    ui: &egui::Ui,
    response: &egui::Response,
    layout: &Layout,
    adjust: Adjust,
) {
    let base = layout.base_points();
    let reach = (HANDLE_REACH / base.x, HANDLE_REACH / base.y);
    let hover = response
        .hover_pos()
        .and_then(|pos| hit_test(adjust.crop, layout.to_frame(pos), reach));
    // Left alone when nothing is under the pointer, so other panels keep their own cursors.
    if let Some(handle) = app.drag.map(|d| d.handle).or(hover) {
        ui.ctx().set_cursor_icon(cursor_for(handle));
    }

    if response.drag_started() {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(press) = press {
            let press_frac = layout.to_frame(press);
            if let Some(handle) = hit_test(adjust.crop, press_frac, reach) {
                app.drag = Some(Drag {
                    handle,
                    start: adjust.crop,
                    press: press_frac,
                });
                app.begin_edit();
            }
        }
    }
    if let Some(drag) = app.drag.filter(|_| response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let now = layout.to_frame(pos);
        let delta = (now.0 - drag.press.0, now.1 - drag.press.1);
        let half = half_extents(layout.base, adjust.angle_deg);
        let crop = drag_frame(drag.start, drag.handle, delta, half, MIN_FRAME);
        app.apply_edit(Adjust { crop, ..adjust });
    }
    if response.drag_stopped() && app.drag.take().is_some() {
        app.commit_edit();
    }
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
