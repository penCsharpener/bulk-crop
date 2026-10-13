//! Right panel: rotation, the fine angle, the view options, the tone and the selected page's settings.

use pagecrop_core::geometry::quarter_turn;
use pagecrop_core::model::Adjust;

use crate::app::App;
use crate::theme;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::right("adjust")
        .resizable(true)
        .default_size(260.0)
        .size_range(240.0..=360.0)
        .show(ui, |ui| {
            ui.heading("Adjust");
            ui.separator();
            rotation(app, ui);
            ui.separator();
            view(app, ui);
            ui.separator();
            tone(app, ui);
            ui.separator();
            page(app, ui);
        });
}

fn rotation(app: &mut App, ui: &mut egui::Ui) {
    theme::section(ui, "Rotation");
    let current = app.effective();
    ui.horizontal(|ui| {
        if ui.button("⟲ 90°").clicked() {
            app.edit_once(quarter_turn(current, false));
        }
        if ui.button("⟳ 90°").clicked() {
            app.edit_once(quarter_turn(current, true));
        }
    });
    let mut angle = current.angle_deg;
    let slider = ui.add(
        egui::Slider::new(&mut angle, -15.0..=15.0)
            .step_by(0.05)
            .suffix("°")
            .text("Fine angle"),
    );
    track(
        app,
        &slider,
        Adjust {
            angle_deg: angle,
            ..current
        },
    );
    if ui.button("0°").clicked() {
        app.edit_once(Adjust {
            angle_deg: 0.0,
            ..current
        });
    }
}

fn tone(app: &mut App, ui: &mut egui::Ui) {
    theme::section(ui, "Tone");
    let current = app.effective();
    let mut brightness = current.brightness;
    let slider = ui.add(egui::Slider::new(&mut brightness, -100..=100).text("Brightness"));
    track(
        app,
        &slider,
        Adjust {
            brightness,
            ..current
        },
    );
    let mut contrast = current.contrast;
    let slider = ui.add(egui::Slider::new(&mut contrast, -100..=100).text("Contrast"));
    track(
        app,
        &slider,
        Adjust {
            contrast,
            ..current
        },
    );
    let mut grey = current.grayscale;
    if ui.checkbox(&mut grey, "Greyscale").changed() {
        app.edit_once(Adjust {
            grayscale: grey,
            ..current
        });
    }
    if ui.button("Reset tone").clicked() {
        app.edit_once(Adjust {
            brightness: 0,
            contrast: 0,
            grayscale: false,
            ..current
        });
    }
}

/// How a control's changes reach the edit helpers. A drag begins with its drag start and commits
/// with its drag stop; a typed value or a click on a track changes the page at once.
fn track(app: &mut App, response: &egui::Response, adjust: Adjust) {
    let at_once = response.changed() && !response.dragged();
    if response.drag_started() || at_once {
        app.begin_edit();
    }
    if response.changed() {
        app.apply_edit(adjust);
    }
    if response.drag_stopped() || at_once {
        app.commit_edit();
    }
}

fn view(app: &mut App, ui: &mut egui::Ui) {
    theme::section(ui, "View");
    ui.checkbox(&mut app.show_grid, "Grid");
    ui.checkbox(&mut app.show_result, "Show result");
}

fn page(app: &mut App, ui: &mut egui::Ui) {
    theme::section(ui, "Page");
    ui.label(app.page_label());
    if ui
        .add_enabled(app.page_has_key(), egui::Button::new("Reset page"))
        .clicked()
    {
        app.reset_page();
    }
}
