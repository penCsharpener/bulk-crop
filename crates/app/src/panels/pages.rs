//! Left panel listing the pages, one row per picture.

use crate::app::App;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::left("pages").exact_size(220.0).show(ui, |ui| {
        ui.heading("Pages");
        ui.separator();
        page_list(app, ui);
    });
}

fn page_list(app: &mut App, ui: &mut egui::Ui) {
    let row_height = ui.spacing().interact_size.y;
    let pitch = row_height + ui.spacing().item_spacing.y;
    let mut scroll = egui::ScrollArea::vertical();
    if app.reveal_selected {
        let centred = app.selected as f32 * pitch - (ui.available_height() - row_height) / 2.0;
        scroll = scroll.vertical_scroll_offset(centred.max(0.0));
        app.reveal_selected = false;
    }
    let count = app.page_count();
    let selected = app.selected;
    let mut clicked = None;
    scroll.show_rows(ui, row_height, count, |ui, range| {
        for index in range {
            let Some(entry) = app.project.as_ref().and_then(|p| p.entries.get(index)) else {
                continue;
            };
            if ui
                .selectable_label(index == selected, entry.file_name.as_str())
                .clicked()
            {
                clicked = Some(index);
            }
        }
    });
    if let Some(index) = clicked {
        app.select(index);
    }
}
