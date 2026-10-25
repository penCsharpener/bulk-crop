//! Right panel: rotation, the fine angle, the view options, the tone and the selected page's settings.

use pagecrop_core::geometry::quarter_turn;
use pagecrop_core::model::Adjust;
use pagecrop_core::tone::ToneSetting;

use crate::app::{App, REACH_RANGE};
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
            sides(app, ui);
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
    if slider.drag_started() {
        app.angle_start = Some(current.crop);
    }
    let turned = app.angled(angle);
    track(app, &slider, turned);
    if slider.drag_stopped() {
        app.angle_start = None;
    }
    if ui.button("0°").clicked() {
        app.edit_once(app.angled(0.0));
    }
    let shifted = current.crop.corner_shift != [(0.0, 0.0); 4];
    if ui
        .add_enabled(shifted, egui::Button::new("Square corners"))
        .on_hover_text(
            "Ctrl + drag a corner of the frame to follow a page photographed at an angle; the export straightens it. This makes the frame a rectangle again.",
        )
        .clicked()
    {
        app.edit_once(app.squared());
    }
}

fn sides(app: &mut App, ui: &mut egui::Ui) {
    theme::section(ui, "Sides");
    let current = app.effective();
    let mut on = current.alternate;
    let toggle = ui.toggle_value(&mut on, theme::arrow_text(ui, "⇄", " Alternating sides")).on_hover_text(
        "Every second page keeps its own frame and angle. Turning it off here affects this page and the pages after it.",
    );
    if toggle.changed() {
        app.edit_once(Adjust {
            alternate: on,
            ..current
        });
    }
    if let Some(label) = app.side_label() {
        ui.label(format!("This page: {label}"));
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
    presets(app, ui, ToneSetting::of(&current));
    if ui.button("Reset tone").clicked() {
        app.end_tone_run();
        app.edit_once(Adjust {
            brightness: 0,
            contrast: 0,
            grayscale: false,
            ..current
        });
    }
}

/// The recent tone settings as buttons; the one the page has now is drawn selected.
fn presets(app: &mut App, ui: &mut egui::Ui, now: ToneSetting) {
    if app.tone_presets.is_empty() {
        return;
    }
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.weak("Recent");
        for tone in &app.tone_presets {
            let button = ui
                .selectable_label(now == *tone, preset_label(*tone))
                .on_hover_text(preset_hint(*tone));
            if button.clicked() {
                picked = Some(*tone);
            }
        }
    });
    if let Some(tone) = picked {
        app.use_tone_preset(tone);
    }
}

/// A value with its sign, using the minus sign; zero has none.
fn signed(v: i32) -> String {
    match v {
        0 => "0".to_owned(),
        v if v < 0 => format!("−{}", -v),
        v => format!("+{v}"),
    }
}

/// "+30 / −10", brightness then contrast, with "grey" after it for greyscale.
fn preset_label(tone: ToneSetting) -> String {
    let base = format!("{} / {}", signed(tone.brightness), signed(tone.contrast));
    if tone.grayscale {
        format!("{base} grey")
    } else {
        base
    }
}

fn preset_hint(tone: ToneSetting) -> String {
    format!(
        "Brightness {}, contrast {}{}",
        signed(tone.brightness),
        signed(tone.contrast),
        if tone.grayscale { ", greyscale" } else { "" },
    )
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
    ui.add(
        egui::Slider::new(&mut app.handle_reach_percent, REACH_RANGE.0..=REACH_RANGE.1)
            .step_by(0.5)
            .suffix(" %")
            .text("Handle reach"),
    )
    .on_hover_text(
        "How close the pointer must be to an edge or corner of the frame, or to a round turn handle, as a share of the picture",
    );
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
