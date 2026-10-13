//! Right panel: rotation, the fine angle, the view options and the settings of the selected page.

use pagecrop_core::geometry::quarter_turn;
use pagecrop_core::model::Adjust;

use crate::app::App;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::right("adjust")
        .exact_size(260.0)
        .show(ui, |ui| {
            ui.heading("Adjust");
            ui.separator();
            rotation(app, ui);
            ui.separator();
            view(app, ui);
            ui.separator();
            page(app, ui);
        });
}

fn rotation(app: &mut App, ui: &mut egui::Ui) {
    ui.strong("Rotation");
    let current = app.effective();
    ui.horizontal(|ui| {
        if ui.button("⟲ 90°").clicked() {
            edit_once(app, quarter_turn(current, false));
        }
        if ui.button("⟳ 90°").clicked() {
            edit_once(app, quarter_turn(current, true));
        }
    });
    let mut angle = current.angle_deg;
    let slider = ui.add(
        egui::Slider::new(&mut angle, -15.0..=15.0)
            .step_by(0.05)
            .suffix("°")
            .text("Fine angle"),
    );
    let typed = slider.changed() && !slider.dragged();
    if slider.drag_started() || typed {
        app.begin_edit();
    }
    if slider.changed() {
        app.apply_edit(Adjust {
            angle_deg: angle,
            ..current
        });
    }
    if slider.drag_stopped() || typed {
        app.commit_edit();
    }
    if ui.button("0°").clicked() {
        edit_once(
            app,
            Adjust {
                angle_deg: 0.0,
                ..current
            },
        );
    }
}

/// One whole edit for a button: begin, apply, commit.
fn edit_once(app: &mut App, adjust: Adjust) {
    app.begin_edit();
    app.apply_edit(adjust);
    app.commit_edit();
}

fn view(app: &mut App, ui: &mut egui::Ui) {
    ui.strong("View");
    ui.checkbox(&mut app.show_grid, "Grid");
    ui.checkbox(&mut app.show_result, "Show result");
}

fn page(app: &mut App, ui: &mut egui::Ui) {
    ui.strong("Page");
    ui.label(app.page_label());
    if ui
        .add_enabled(app.page_has_key(), egui::Button::new("Reset page"))
        .clicked()
    {
        app.reset_page();
    }
}
