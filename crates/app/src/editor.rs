//! Central area: the selected page turned by its settings, with its crop frame on top.

use pagecrop_core::crop_edit::{
    Handle, drag_corner, drag_inside, handle_reach, hit_test, spin_angle, spin_reach,
};
use pagecrop_core::geometry::{Resize, Size, SquareToQuad, base_size, frame_corners};
use pagecrop_core::model::{Adjust, CropRect};
use pagecrop_core::render::render;

use crate::app::{App, Content, Drag, ResultView, SpinDrag, ToneView};
use crate::theme::ACCENT;

/// The smallest frame edge, as a fraction of the base picture.
const MIN_FRAME: (f32, f32) = (0.02, 0.02);
/// How close the pointer must be to a handle, in screen points.
const HANDLE_REACH: f32 = 8.0;
/// The largest zoom: the picture can be magnified up to this many times its fitted size.
const MAX_ZOOM: f32 = 8.0;
const HANDLE_SIZE: f32 = 9.0;
/// The radius of a spin handle's dot, in screen points.
const SPIN_RADIUS: f32 = 5.0;
/// The least distance from a spin handle's dot at which the pointer is on it, in screen points. The
/// "Handle reach" setting can make it larger.
const SPIN_REACH: f32 = 10.0;
/// The nearest a spin handle sits to the frame edge, in screen points.
const SPIN_MIN_DISTANCE: f32 = 28.0;
/// The gap between a spin handle's grab circle and the frame handles' reach, in screen points.
const SPIN_GAP: f32 = 4.0;
/// How near the canvas edge a spin handle is pulled back to, in screen points.
const SPIN_MARGIN: f32 = 10.0;
/// The fine angle's range in degrees, the same as the slider's.
const ANGLE_LIMIT: f32 = 15.0;

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
    if app.show_result {
        show_result(app, ui, area);
        return;
    }
    let adjust = app.effective();
    let texture = toned_texture(app, ui.ctx(), texture, adjust);
    let Some(layout) = Layout::new(&texture, adjust, area, app.zoom_factor, app.pan) else {
        return;
    };
    let painter = ui.painter_at(area);
    paint_picture(&painter, &texture, adjust, &layout);
    paint_frame(&painter, &layout, area, adjust.crop, app.show_grid);
    let (dots, _) = spin_positions(app, &layout, area, adjust.crop);
    paint_spin_handles(&painter, layout.frame_rect(adjust.crop), dots);
    if let Some(label) = app.side_label() {
        paint_side_badge(&painter, area, label);
    }
    view_input(app, ui, &response, &layout, adjust.crop);
    handle_pointer(app, ui, &response, &layout, adjust);
}

/// The preview with the tone applied, when the tone is not the default. It is rebuilt only when the
/// page or the tone values change; otherwise the plain texture is used.
fn toned_texture(
    app: &mut App,
    ctx: &egui::Context,
    plain: &egui::TextureHandle,
    adjust: Adjust,
) -> egui::TextureHandle {
    if adjust.brightness == 0 && adjust.contrast == 0 && !adjust.grayscale {
        return plain.clone();
    }
    let key = (
        app.selected,
        app.preview_generation,
        adjust.brightness,
        adjust.contrast,
        adjust.grayscale,
    );
    if let Some(tone) = &app.tone
        && tone.key == key
    {
        return tone.texture.clone();
    }
    let Some(Content::Picture { rgb, .. }) = app.selected_content() else {
        return plain.clone();
    };
    let tone_only = Adjust {
        brightness: adjust.brightness,
        contrast: adjust.contrast,
        grayscale: adjust.grayscale,
        ..Adjust::default()
    };
    let toned = render(rgb, &tone_only, Resize::None).to_rgb8();
    let size = [toned.width() as usize, toned.height() as usize];
    let color = egui::ColorImage::from_rgb(size, toned.as_raw());
    let texture = ctx.load_texture("tone", color, egui::TextureOptions::LINEAR);
    app.tone = Some(ToneView {
        key,
        texture: texture.clone(),
    });
    texture
}

/// The selected page rendered as it will be exported, fitted into the area. The render is kept, and
/// redone only when the page, its settings or the view's size change.
fn show_result(app: &mut App, ui: &egui::Ui, area: egui::Rect) {
    let adjust = app.effective();
    let longest = area.width().max(area.height()).round().max(1.0) as u32;
    let key = (app.selected, app.preview_generation, adjust, longest);
    if app.result.as_ref().is_none_or(|r| r.key != key) {
        let Some(Content::Picture { rgb, .. }) = app.selected_content() else {
            return;
        };
        let out = render(rgb, &adjust, Resize::LongEdge(longest)).to_rgb8();
        let size = [out.width() as usize, out.height() as usize];
        let color = egui::ColorImage::from_rgb(size, out.as_raw());
        let texture = ui
            .ctx()
            .load_texture("result", color, egui::TextureOptions::LINEAR);
        app.result = Some(ResultView { key, texture });
    }
    if let Some(result) = &app.result {
        paint_fitted(
            ui.painter(),
            &result.texture,
            area.shrink2(area.size() * 0.04),
        );
    }
}

/// Where the picture sits on screen, and how frame fractions map to screen points.
struct Layout {
    /// The centre of the fitted area: where the picture is centred when it is not panned.
    fit_centre: egui::Pos2,
    /// Where the centre of the picture is now: `fit_centre` moved by the pan.
    centre: egui::Pos2,
    /// Screen points per preview pixel, zoom included.
    scale: f32,
    /// The preview after its quarter turns, in preview pixels.
    base: Size,
    /// The preview texture's size, in preview pixels.
    texture: egui::Vec2,
}

impl Layout {
    /// Fits the base picture into the area with a 4 % margin, then applies the zoom and the pan.
    /// None when there is nothing to draw.
    fn new(
        texture: &egui::TextureHandle,
        adjust: Adjust,
        area: egui::Rect,
        zoom: f32,
        pan: egui::Vec2,
    ) -> Option<Layout> {
        let [w, h] = texture.size().map(|v| v as f32);
        let base = base_size(Size { w, h }, adjust.quarter_turns);
        let inner = area.shrink2(area.size() * 0.04);
        let scale = (inner.width() / base.w).min(inner.height() / base.h) * zoom;
        if base.w < 1.0 || base.h < 1.0 || scale.is_nan() || scale <= 0.0 {
            return None;
        }
        let fit_centre = inner.center();
        Some(Layout {
            fit_centre,
            centre: fit_centre + pan,
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

    /// The upright rectangle of the base picture on screen: where the turned picture is clipped.
    fn base_rect(&self) -> egui::Rect {
        egui::Rect::from_center_size(self.centre, self.base_points())
    }

    /// The four corners of the crop frame on screen, NW, NE, SE, SW.
    fn frame_points(&self, crop: CropRect) -> [egui::Pos2; 4] {
        frame_corners(crop).map(|corner| self.to_screen(corner))
    }

    /// The upright box around the crop frame on screen: the frame itself when it is a rectangle.
    fn frame_rect(&self, crop: CropRect) -> egui::Rect {
        egui::Rect::from_points(&self.frame_points(crop))
    }
}

/// Draws the picture turned by its settings. The fine angle turns it further. The picture is cut at
/// the upright base rectangle, and the wedges the turn leaves there are white, as in the export.
fn paint_picture(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    adjust: Adjust,
    layout: &Layout,
) {
    painter.rect_filled(layout.base_rect(), 0.0, egui::Color32::WHITE);
    let rect = egui::Rect::from_center_size(layout.centre, layout.texture * layout.scale);
    let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let mut mesh = egui::Mesh::with_texture(texture.id());
    mesh.add_rect_with_uv(rect, full_uv, egui::Color32::WHITE);
    let degrees = f32::from(adjust.quarter_turns) * 90.0 + adjust.angle_deg;
    mesh.rotate(
        egui::emath::Rot2::from_angle(degrees.to_radians()),
        layout.centre,
    );
    painter
        .with_clip_rect(layout.base_rect())
        .add(egui::Shape::mesh(mesh));
}

/// Shades what lies outside the frame and draws the frame, its handles and the optional grid.
fn paint_frame(
    painter: &egui::Painter,
    layout: &Layout,
    area: egui::Rect,
    crop: CropRect,
    grid: bool,
) {
    let points = layout.frame_points(crop);
    let shade = egui::Color32::from_black_alpha(128);
    let outer = outer_ring(area, points);
    painter.add(egui::Shape::mesh(shade_mesh(outer, points, shade)));
    if grid {
        let stroke = egui::Stroke::new(0.5, egui::Color32::from_white_alpha(140));
        for line in grid_lines(points) {
            painter.line_segment(line, stroke);
        }
    }
    let outline = points.to_vec();
    painter.add(egui::Shape::closed_line(
        outline.clone(),
        egui::Stroke::new(2.5, egui::Color32::from_black_alpha(200)),
    ));
    painter.add(egui::Shape::closed_line(
        outline,
        egui::Stroke::new(1.5, egui::Color32::WHITE),
    ));
    for point in handle_points(points) {
        let handle = egui::Rect::from_center_size(point, egui::vec2(HANDLE_SIZE, HANDLE_SIZE));
        painter.rect_filled(handle, 1.0, ACCENT);
    }
}

/// The corners, NW, NE, SE, SW, of a rectangle that contains both `area` and the frame `points`.
fn outer_ring(area: egui::Rect, points: [egui::Pos2; 4]) -> [egui::Pos2; 4] {
    let rect = area.union(egui::Rect::from_points(&points));
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

/// The ring between two quadrilaterals as one mesh, `outer` around `inner`, both in the order NW, NE,
/// SE, SW. Vertices 0 to 3 are the outer ring and 4 to 7 the inner one. Each side is two triangles:
/// outer i, outer i+1, inner i+1 and outer i, inner i+1, inner i.
fn shade_mesh(outer: [egui::Pos2; 4], inner: [egui::Pos2; 4], color: egui::Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for point in outer.into_iter().chain(inner) {
        mesh.colored_vertex(point, color);
    }
    for i in 0..4u32 {
        let next = (i + 1) % 4;
        mesh.add_triangle(i, next, 4 + next);
        mesh.add_triangle(i, 4 + next, 4 + i);
    }
    mesh
}

/// The grid lines of the frame `points` (NW, NE, SE, SW): seven lines from top to bottom and then
/// seven across, each an eighth of the way further, drawn through the same map the export uses so
/// they meet where the page's own lines will be straight.
fn grid_lines(points: [egui::Pos2; 4]) -> Vec<[egui::Pos2; 2]> {
    let map = SquareToQuad::new(points.map(|p| (p.x, p.y)));
    let at = |s: f32, t: f32| {
        let (x, y) = map.apply(s, t);
        egui::pos2(x, y)
    };
    let steps = (1..=7).map(|k| k as f32 / 8.0);
    let columns = steps.clone().map(|k| [at(k, 0.0), at(k, 1.0)]);
    let rows = steps.map(|k| [at(0.0, k), at(1.0, k)]);
    columns.chain(rows).collect()
}

/// Where the two spin handles are: the left and the right of the frame, at its vertical middle.
/// Each is `max(28, reach_x + radius + 4)` points from its edge, so its grab circle (`radius`) lies
/// beyond the E and W handles' reach (`reach_x`, in points). A dot that would leave `area` is pulled
/// back to 10 points inside.
fn spin_dots(frame: egui::Rect, reach_x: f32, radius: f32, area: egui::Rect) -> [egui::Pos2; 2] {
    let distance = SPIN_MIN_DISTANCE.max(reach_x + radius + SPIN_GAP);
    let y = frame.center().y;
    let (lo, hi) = (area.min.x + SPIN_MARGIN, area.max.x - SPIN_MARGIN);
    [
        egui::pos2(clamp_range(frame.min.x - distance, lo, hi), y),
        egui::pos2(clamp_range(frame.max.x + distance, lo, hi), y),
    ]
}

/// The grab radius of each spin handle. It is `radius`, but where the dot was pulled back towards
/// its edge it shrinks to leave `SPIN_GAP` points before the frame edge, so the edge itself always
/// belongs to the E or W handle; it is never less than `SPIN_REACH`.
fn spin_radii(frame: egui::Rect, dots: [egui::Pos2; 2], radius: f32) -> [f32; 2] {
    let room = |dot: egui::Pos2, edge: f32| (dot.x - edge).abs() - SPIN_GAP;
    [
        radius.min(room(dots[0], frame.min.x).max(SPIN_REACH)),
        radius.min(room(dots[1], frame.max.x).max(SPIN_REACH)),
    ]
}

/// Clamps `v` into `lo..=hi`; unlike `f32::clamp` it never panics on a tiny area where `lo > hi`.
fn clamp_range(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// The spin handles of the current frame and handle reach, and the radius in points around each
/// within which the pointer is on it.
fn spin_positions(
    app: &App,
    layout: &Layout,
    area: egui::Rect,
    crop: CropRect,
) -> ([egui::Pos2; 2], [f32; 2]) {
    let base = layout.base_points();
    let radius = spin_reach((base.x, base.y), app.handle_reach_percent, SPIN_REACH);
    let reach_x = reach(layout, crop, app.handle_reach_percent).0 * base.x;
    let frame = layout.frame_rect(crop);
    let dots = spin_dots(frame, reach_x, radius, area);
    (dots, spin_radii(frame, dots, radius))
}

/// Whether `pos` is within the radius of one of the spin handles.
fn on_spin_dot(dots: [egui::Pos2; 2], radii: [f32; 2], pos: egui::Pos2) -> bool {
    dots.iter()
        .zip(radii)
        .any(|(dot, radius)| dot.distance(pos) <= radius)
}

/// The two spin handles: a dot on each side of the frame, joined to the middle of that edge by a
/// thin line.
fn paint_spin_handles(painter: &egui::Painter, frame: egui::Rect, dots: [egui::Pos2; 2]) {
    let edges = [frame.left_center(), frame.right_center()];
    for (dot, edge) in dots.into_iter().zip(edges) {
        painter.line_segment(
            [edge, dot],
            egui::Stroke::new(2.0, egui::Color32::from_black_alpha(160)),
        );
        painter.line_segment([edge, dot], egui::Stroke::new(1.0, egui::Color32::WHITE));
        painter.circle_filled(dot, SPIN_RADIUS, ACCENT);
        painter.circle_stroke(
            dot,
            SPIN_RADIUS,
            egui::Stroke::new(1.0, egui::Color32::from_gray(30)),
        );
    }
}

/// "Side A" or "Side B" in the top-left corner of the area, on a half-transparent dark rectangle.
fn paint_side_badge(painter: &egui::Painter, area: egui::Rect, side: &str) {
    let text = side.replacen("side", "Side", 1);
    let galley =
        painter.layout_no_wrap(text, egui::FontId::proportional(14.0), egui::Color32::WHITE);
    let rect = egui::Rect::from_min_size(
        area.min + egui::vec2(8.0, 8.0),
        galley.size() + egui::vec2(16.0, 8.0),
    );
    painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(160));
    painter.galley(
        rect.min + egui::vec2(8.0, 4.0),
        galley,
        egui::Color32::WHITE,
    );
}

/// The eight handles: the corners NW, NE, SE, SW, then the middle of each edge N, E, S, W.
fn handle_points(p: [egui::Pos2; 4]) -> [egui::Pos2; 8] {
    let middle = |a: egui::Pos2, b: egui::Pos2| a + (b - a) * 0.5;
    [
        p[0],
        p[1],
        p[2],
        p[3],
        middle(p[0], p[1]),
        middle(p[1], p[2]),
        middle(p[3], p[2]),
        middle(p[0], p[3]),
    ]
}

/// Whether `handle` is one of the four corners, which Ctrl + drag moves alone.
fn is_corner(handle: Handle) -> bool {
    matches!(handle, Handle::NW | Handle::NE | Handle::SE | Handle::SW)
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

/// How close the pointer must be to a handle, as fractions of the base picture: the user's setting,
/// but never less than `HANDLE_REACH` screen points.
fn reach(layout: &Layout, crop: CropRect, percent: f32) -> (f32, f32) {
    let base = layout.base_points();
    let floor = (HANDLE_REACH / base.x, HANDLE_REACH / base.y);
    handle_reach(layout.base, crop, percent, floor)
}

/// Wheel zoom around the pointer, and pan with the middle button or with Space and the left button.
/// A double click on empty canvas returns to the fitted view.
fn view_input(
    app: &mut App,
    ui: &egui::Ui,
    response: &egui::Response,
    layout: &Layout,
    crop: CropRect,
) {
    let mut changed = false;
    let (dots, radii) = spin_positions(app, layout, response.rect, crop);
    let scroll = if response.hovered() {
        ui.input(|i| i.smooth_scroll_delta.y)
    } else {
        0.0
    };
    if scroll != 0.0
        && let Some(pointer) = response.hover_pos()
    {
        zoom_about(app, layout, pointer, (scroll / 200.0).exp());
        changed = true;
    }
    if panning(ui, response) {
        app.pan += response.drag_delta();
        changed = true;
    }
    let on_handle = response.hover_pos().is_some_and(|pos| {
        on_spin_dot(dots, radii, pos)
            || hit_test(
                crop,
                layout.to_frame(pos),
                reach(layout, crop, app.handle_reach_percent),
            )
            .is_some()
    });
    if response.double_clicked() && !on_handle {
        app.reset_view();
        changed = true;
    }
    if changed {
        ui.ctx().request_repaint();
    }
}

/// Changes the zoom by `factor`, keeping the point under `pointer` where it is on screen.
fn zoom_about(app: &mut App, layout: &Layout, pointer: egui::Pos2, factor: f32) {
    let zoom = (app.zoom_factor * factor).clamp(1.0, MAX_ZOOM);
    let ratio = zoom / app.zoom_factor;
    let centre = pointer + (layout.centre - pointer) * ratio;
    app.zoom_factor = zoom;
    app.pan = if zoom <= 1.0 {
        egui::Vec2::ZERO
    } else {
        centre - layout.fit_centre
    };
}

/// Whether the pointer pans the picture: the middle button, or the left button while Space is held.
fn panning(ui: &egui::Ui, response: &egui::Response) -> bool {
    response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary)
            && ui.input(|i| i.key_down(egui::Key::Space)))
}

fn handle_pointer(
    app: &mut App,
    ui: &egui::Ui,
    response: &egui::Response,
    layout: &Layout,
    adjust: Adjust,
) {
    let reach = reach(layout, adjust.crop, app.handle_reach_percent);
    let hover = response
        .hover_pos()
        .and_then(|pos| hit_test(adjust.crop, layout.to_frame(pos), reach));
    // Space turns a left drag into a pan, so it never starts a handle or spin drag.
    let (space, ctrl) = ui.input(|i| (i.key_down(egui::Key::Space), i.modifiers.command));
    let (dots, radii) = spin_positions(app, layout, response.rect, adjust.crop);
    let on_spin = !space
        && response
            .hover_pos()
            .is_some_and(|pos| on_spin_dot(dots, radii, pos));
    // Left alone when nothing is under the pointer, so other panels keep their own cursors.
    if app.spin.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if on_spin {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    } else if app.drag.is_some_and(|d| d.corner) || (ctrl && hover.is_some_and(is_corner)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    } else if let Some(handle) = app.drag.map(|d| d.handle).or(hover) {
        ui.ctx().set_cursor_icon(cursor_for(handle));
    }

    if response.drag_started_by(egui::PointerButton::Primary) && !space {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(press) = press {
            // The spin handles are checked before the frame.
            if on_spin_dot(dots, radii, press) {
                app.spin = Some(SpinDrag {
                    start_deg: adjust.angle_deg,
                    press,
                });
                app.angle_start = Some(adjust.crop);
                app.begin_edit();
                return;
            }
            let press_frac = layout.to_frame(press);
            if let Some(handle) = hit_test(adjust.crop, press_frac, reach) {
                app.drag = Some(Drag {
                    handle,
                    start: adjust.crop,
                    press: press_frac,
                    corner: ctrl && is_corner(handle),
                });
                app.begin_edit();
            }
        }
    }
    if let Some(spin) = app.spin
        && response.dragged_by(egui::PointerButton::Primary)
        && let Some(pos) = response.interact_pointer_pos()
    {
        let centre = (layout.centre.x, layout.centre.y);
        let angle = spin_angle(
            spin.start_deg,
            centre,
            (spin.press.x, spin.press.y),
            (pos.x, pos.y),
            ANGLE_LIMIT,
        );
        app.apply_edit(app.angled(angle));
    }
    if let Some(drag) = app
        .drag
        .filter(|_| response.dragged_by(egui::PointerButton::Primary))
        && let Some(pos) = response.interact_pointer_pos()
    {
        let now = layout.to_frame(pos);
        let delta = (now.0 - drag.press.0, now.1 - drag.press.1);
        let crop = if drag.corner {
            drag_corner(
                drag.start,
                drag.handle,
                delta,
                layout.base,
                adjust.angle_deg,
            )
        } else {
            drag_inside(
                drag.start,
                drag.handle,
                delta,
                layout.base,
                adjust.angle_deg,
                MIN_FRAME,
            )
        };
        app.apply_edit(Adjust { crop, ..adjust });
    }
    if response.drag_stopped() {
        if app.spin.take().is_some() {
            app.angle_start = None;
            app.commit_edit();
        } else if app.drag.take().is_some() {
            app.commit_edit();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 800.0))
    }

    fn frame() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(300.0, 100.0), egui::pos2(700.0, 500.0))
    }

    #[test]
    fn dots_sit_at_the_vertical_middle_outside_the_edges() {
        let [left, right] = spin_dots(frame(), 10.0, 10.0, area());
        assert!((left.x - 272.0).abs() < 1e-3, "left = {left:?}");
        assert!((right.x - 728.0).abs() < 1e-3, "right = {right:?}");
        assert!((left.y - 300.0).abs() < 1e-3 && (right.y - 300.0).abs() < 1e-3);
    }

    #[test]
    fn dots_stay_beyond_a_wide_reach() {
        let [left, right] = spin_dots(frame(), 60.0, 10.0, area());
        assert!((left.x - 226.0).abs() < 1e-3, "left = {left:?}");
        assert!((right.x - 774.0).abs() < 1e-3, "right = {right:?}");
    }

    #[test]
    fn dots_move_out_with_the_grab_radius() {
        let [left, right] = spin_dots(frame(), 30.0, 30.0, area());
        assert!((left.x - 236.0).abs() < 1e-3, "left = {left:?}");
        assert!((right.x - 764.0).abs() < 1e-3, "right = {right:?}");
    }

    #[test]
    fn radius_stays_whole_where_the_dot_is_not_pulled_in() {
        let dots = spin_dots(frame(), 30.0, 30.0, area());
        let radii = spin_radii(frame(), dots, 30.0);
        assert!(radii.iter().all(|r| (r - 30.0).abs() < 1e-3), "{radii:?}");
    }

    #[test]
    fn radius_stops_short_of_the_frame_edge() {
        // A dot 50 points from its edge leaves 46 before the edge, with a gap of 4.
        let dots = [egui::pos2(250.0, 300.0), egui::pos2(750.0, 300.0)];
        let radii = spin_radii(frame(), dots, 60.0);
        assert!(radii.iter().all(|r| (r - 46.0).abs() < 1e-3), "{radii:?}");
    }

    #[test]
    fn radius_never_drops_below_the_floor() {
        let wide = egui::Rect::from_min_max(egui::pos2(5.0, 100.0), egui::pos2(995.0, 500.0));
        let dots = spin_dots(wide, 30.0, 30.0, area());
        let radii = spin_radii(wide, dots, 30.0);
        assert!(
            radii.iter().all(|r| (r - SPIN_REACH).abs() < 1e-3),
            "{radii:?}"
        );
    }

    #[test]
    fn dots_are_pulled_back_into_the_canvas() {
        let wide = egui::Rect::from_min_max(egui::pos2(5.0, 100.0), egui::pos2(995.0, 500.0));
        let [left, right] = spin_dots(wide, 10.0, 10.0, area());
        assert!((left.x - 10.0).abs() < 1e-3, "left = {left:?}");
        assert!((right.x - 990.0).abs() < 1e-3, "right = {right:?}");
    }

    /// A layout whose base picture is 1000 x 800 points centred at (500, 400).
    fn layout() -> Layout {
        Layout {
            fit_centre: egui::pos2(500.0, 400.0),
            centre: egui::pos2(500.0, 400.0),
            scale: 1.0,
            base: Size {
                w: 1000.0,
                h: 800.0,
            },
            texture: egui::vec2(1000.0, 800.0),
        }
    }

    fn trapezoid() -> CropRect {
        CropRect {
            cx: 0.0,
            cy: 0.0,
            w: 0.6,
            h: 0.5,
            corner_shift: [(0.1, 0.0), (-0.1, 0.0), (0.0, 0.0), (0.0, 0.0)],
        }
    }

    fn near(a: egui::Pos2, x: f32, y: f32) -> bool {
        (a.x - x).abs() < 1e-3 && (a.y - y).abs() < 1e-3
    }

    #[test]
    fn frame_points_follow_the_shift() {
        let p = layout().frame_points(trapezoid());
        // NW is the rectangle's corner (-0.3, -0.25) moved right by 0.1 of the base width.
        assert!(near(p[0], 500.0 - 200.0, 400.0 - 200.0), "{p:?}");
        assert!(near(p[1], 500.0 + 200.0, 400.0 - 200.0), "{p:?}");
        assert!(near(p[2], 500.0 + 300.0, 400.0 + 200.0), "{p:?}");
        assert!(near(p[3], 500.0 - 300.0, 400.0 + 200.0), "{p:?}");
    }

    #[test]
    fn frame_rect_is_the_bounding_box() {
        let rect = layout().frame_rect(trapezoid());
        assert!(
            near(rect.min, 200.0, 200.0) && near(rect.max, 800.0, 600.0),
            "{rect:?}"
        );
    }

    #[test]
    fn plain_frame_points_are_the_rectangle_corners() {
        let plain = CropRect {
            corner_shift: [(0.0, 0.0); 4],
            ..trapezoid()
        };
        let p = layout().frame_points(plain);
        let rect = layout().frame_rect(plain);
        assert!(near(p[0], rect.min.x, rect.min.y));
        assert!(near(p[2], rect.max.x, rect.max.y));
    }

    /// Twice the signed area of a triangle; positive when it runs clockwise on screen.
    fn cross(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }

    fn quad_area(p: [egui::Pos2; 4]) -> f32 {
        (cross(p[0], p[1], p[2]) + cross(p[0], p[2], p[3])) / 2.0
    }

    #[test]
    fn outer_ring_contains_area_and_frame() {
        let points = layout().frame_points(trapezoid());
        let outer = outer_ring(area(), points);
        assert!(
            near(outer[0], 0.0, 0.0) && near(outer[2], 1000.0, 800.0),
            "{outer:?}"
        );
        let big = [
            egui::pos2(-50.0, 100.0),
            egui::pos2(300.0, 100.0),
            egui::pos2(300.0, 900.0),
            egui::pos2(-50.0, 900.0),
        ];
        let outer = outer_ring(area(), big);
        assert!(
            near(outer[0], -50.0, 0.0) && near(outer[2], 1000.0, 900.0),
            "{outer:?}"
        );
    }

    #[test]
    fn shade_covers_exactly_the_ring() {
        let inner = layout().frame_points(trapezoid());
        let outer = outer_ring(area(), inner);
        let mesh = shade_mesh(outer, inner, egui::Color32::BLACK);
        assert_eq!(mesh.indices.len(), 24, "two triangles per side");
        let mut area_sum = 0.0;
        for tri in mesh.indices.chunks(3) {
            let at = |i: u32| mesh.vertices[i as usize].pos;
            let signed = cross(at(tri[0]), at(tri[1]), at(tri[2])) / 2.0;
            assert!(signed >= -1e-3, "triangle {tri:?} runs the wrong way");
            area_sum += signed;
        }
        let expected = quad_area(outer) - quad_area(inner);
        assert!(
            (area_sum - expected).abs() < 1.0,
            "{area_sum} vs {expected}"
        );
    }

    #[test]
    fn shade_uses_the_documented_triangles() {
        let inner = layout().frame_points(trapezoid());
        let outer = outer_ring(area(), inner);
        let mesh = shade_mesh(outer, inner, egui::Color32::BLACK);
        // Vertices 0..4 are the outer ring, 4..8 the inner ring, both NW, NE, SE, SW.
        assert_eq!(&mesh.indices[..6], &[0, 1, 5, 0, 5, 4]);
        assert_eq!(&mesh.indices[18..], &[3, 0, 4, 3, 4, 7]);
    }

    #[test]
    fn grid_leans_with_the_sides() {
        let points = layout().frame_points(trapezoid());
        let lines = grid_lines(points);
        assert_eq!(lines.len(), 14);
        // Line 1 of the columns runs from the top edge to the bottom edge, slanting like the sides.
        let [top, bottom] = lines[0];
        assert!((top.y - 200.0).abs() < 1e-2 && (bottom.y - 600.0).abs() < 1e-2);
        assert!(top.x > 300.0 && top.x < 500.0 && bottom.x > 200.0 && bottom.x < 500.0);
        assert!(
            top.x - bottom.x > 0.0,
            "the left column leans in at the top"
        );
    }

    #[test]
    fn grid_of_a_rectangle_is_straight() {
        let plain = CropRect {
            corner_shift: [(0.0, 0.0); 4],
            ..trapezoid()
        };
        let lines = grid_lines(layout().frame_points(plain));
        // Columns first: line 4 of 8 is the vertical middle, rows follow.
        let [top, bottom] = lines[3];
        assert!(
            near(top, 500.0, 200.0) && near(bottom, 500.0, 600.0),
            "{top:?} {bottom:?}"
        );
        let [left, right] = lines[7 + 3];
        assert!(
            near(left, 200.0, 400.0) && near(right, 800.0, 400.0),
            "{left:?} {right:?}"
        );
    }

    #[test]
    fn handles_sit_on_the_corners_and_edge_middles() {
        let points = layout().frame_points(trapezoid());
        let h = handle_points(points);
        assert!(near(h[0], points[0].x, points[0].y) && near(h[3], points[3].x, points[3].y));
        // N, E, S, W follow, each halfway between its two corners.
        assert!(near(h[4], 500.0, 200.0), "{:?}", h[4]);
        assert!(near(h[5], 750.0, 400.0), "{:?}", h[5]);
        assert!(near(h[6], 500.0, 600.0), "{:?}", h[6]);
        assert!(near(h[7], 250.0, 400.0), "{:?}", h[7]);
    }
}
