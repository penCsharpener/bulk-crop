//! Left panel listing the pages: a thumbnail and the file name for each.

use egui::{Align2, Color32, FontId, Rect, Sense};

use crate::app::App;
use crate::editor::paint_fitted;
use crate::theme::ACCENT;

/// Height of one page row, in points.
const ROW_HEIGHT: f32 = 96.0;
const THUMB_W: f32 = 120.0;
const THUMB_H: f32 = 84.0;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::left("pages")
        .resizable(true)
        .default_size(220.0)
        .size_range(160.0..=360.0)
        .show(ui, |ui| {
            ui.heading("Pages");
            ui.separator();
            page_list(app, ui);
        });
}

fn page_list(app: &mut App, ui: &mut egui::Ui) {
    let pitch = ROW_HEIGHT + ui.spacing().item_spacing.y;
    let mut scroll = egui::ScrollArea::vertical();
    if app.reveal_selected {
        let centred = app.selected as f32 * pitch - (ui.available_height() - ROW_HEIGHT) / 2.0;
        scroll = scroll.vertical_scroll_offset(centred.max(0.0));
        app.reveal_selected = false;
    }
    let count = app.page_count();
    let selected = app.selected;
    let mut clicked = None;
    scroll.show_rows(ui, ROW_HEIGHT, count, |ui, range| {
        for index in range {
            if page_row(ui, app, index, index == selected) {
                clicked = Some(index);
            }
        }
    });
    if let Some(index) = clicked {
        app.select(index);
    }
}

/// One row: the thumbnail, or a grey box until it arrives, and the file name. True when clicked.
fn page_row(ui: &mut egui::Ui, app: &App, index: usize, selected: bool) -> bool {
    let Some(entry) = app.project.as_ref().and_then(|p| p.entries.get(index)) else {
        return false;
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
        // The accent bar on the left edge marks the page on screen.
        let bar = Rect::from_min_size(rect.left_top(), egui::vec2(4.0, rect.height()));
        painter.rect_filled(bar, 2.0, ACCENT);
    }
    let thumb_box = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + THUMB_H / 2.0),
        egui::vec2(THUMB_W, THUMB_H),
    );
    match app.thumbs.get(index).and_then(|t| t.as_ref()) {
        Some(texture) => paint_fitted(painter, texture, thumb_box),
        None => {
            painter.rect_filled(thumb_box, 2.0, Color32::from_gray(60));
        }
    }
    if entry.key.is_some() {
        painter.circle_filled(
            egui::pos2(rect.left() + 12.0, rect.top() + 8.0),
            3.5,
            ACCENT,
        );
    }
    if let Some(side) = app.project.as_ref().and_then(|p| p.side(index)) {
        let (text, color) = if side == 0 {
            ("A", ui.visuals().text_color())
        } else {
            ("B", ACCENT)
        };
        painter.text(
            egui::pos2(rect.right() - 6.0, rect.top() + 4.0),
            Align2::RIGHT_TOP,
            text,
            FontId::proportional(12.0),
            color,
        );
    }
    painter.text(
        egui::pos2(rect.center().x, rect.top() + THUMB_H),
        Align2::CENTER_TOP,
        entry.file_name.as_str(),
        FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
    response.clicked()
}
