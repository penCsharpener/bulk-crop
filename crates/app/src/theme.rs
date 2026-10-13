//! The look of the window: a flat dark style, one accent colour, and the window icon.

use egui::{Color32, CornerRadius, FontId, IconData, RichText, Stroke, TextStyle, Ui, Visuals};

/// The one accent colour: selection, the dots of pages with settings, and the frame handles.
pub const ACCENT: Color32 = Color32::from_rgb(64, 156, 255);
/// Secondary text, such as section headings.
const DIM: Color32 = Color32::from_gray(140);
const PANEL: Color32 = Color32::from_rgb(18, 18, 20);
const WINDOW: Color32 = Color32::from_rgb(26, 26, 30);
const WIDGET: Color32 = Color32::from_rgb(40, 40, 46);
const WIDGET_HOVER: Color32 = Color32::from_rgb(56, 56, 64);
const WIDGET_ACTIVE: Color32 = Color32::from_rgb(70, 70, 80);
const ROUNDING: u8 = 6;

/// Sets the flat dark style, the spacing and the text sizes on every style the window can use.
pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.visuals = visuals();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        for (text_style, size) in [
            (TextStyle::Heading, 20.0),
            (TextStyle::Body, 15.0),
            (TextStyle::Button, 15.0),
            (TextStyle::Small, 12.0),
        ] {
            style
                .text_styles
                .insert(text_style, FontId::proportional(size));
        }
        style
            .text_styles
            .insert(TextStyle::Monospace, FontId::monospace(14.0));
    });
}

fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = WINDOW;
    visuals.extreme_bg_color = Color32::from_rgb(12, 12, 14);
    visuals.window_corner_radius = CornerRadius::same(ROUNDING);
    visuals.menu_corner_radius = CornerRadius::same(ROUNDING);
    visuals.selection.bg_fill = Color32::from_rgb(34, 52, 76);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    let widgets = &mut visuals.widgets;
    for (style, fill) in [
        (&mut widgets.noninteractive, WIDGET),
        (&mut widgets.inactive, WIDGET),
        (&mut widgets.hovered, WIDGET_HOVER),
        (&mut widgets.active, WIDGET_ACTIVE),
        (&mut widgets.open, WIDGET),
    ] {
        style.weak_bg_fill = fill;
        style.bg_fill = fill;
        style.corner_radius = CornerRadius::same(ROUNDING);
    }
    visuals
}

/// A heading for a section of the right panel: small capitals in a dim colour.
pub fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title.to_uppercase()).small().color(DIM));
}

/// The window icon: a white page on an accent tile, with three lines of text.
pub fn window_icon() -> IconData {
    const SIZE: u32 = 64;
    let [ar, ag, ab, _] = ACCENT.to_array();
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on_page = (16..48).contains(&x) && (10..54).contains(&y);
            let is_line = on_page && (22..42).contains(&x) && (y == 24 || y == 32 || y == 40);
            let pixel = if is_line {
                [90, 90, 96, 255]
            } else if on_page {
                [245, 245, 240, 255]
            } else {
                [ar, ag, ab, 255]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}
