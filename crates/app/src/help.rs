//! The window that lists every shortcut, opened with F1 or the ? button.

use crate::app::App;

/// Each shortcut or mouse action, and what it does. Keep it in step with `App::handle_keys` and
/// the editor.
const SHORTCUTS: &[(&str, &str)] = &[
    ("Right, Down, Page Down", "Next page"),
    ("Left, Up, Page Up", "Previous page"),
    ("Home, End", "First page, last page"),
    ("N, Shift+N", "Next, previous page with its own settings"),
    ("R, Shift+R", "Turn 90° clockwise, anticlockwise"),
    ("[ and ]", "Fine angle −0.1°, +0.1°"),
    ("Shift+[ and Shift+]", "Fine angle −1°, +1°"),
    ("Delete", "Reset the page to the settings before it"),
    ("Shift+Delete", "Move the photo to the trash"),
    ("G", "Show or hide the grid"),
    ("P", "Show the result, or the frame again"),
    ("0", "Fit the picture to the window"),
    ("Mouse wheel", "Zoom in or out around the pointer"),
    ("Middle drag, or Space + drag", "Pan the zoomed picture"),
    (
        "Double click on empty picture",
        "Fit the picture to the window",
    ),
    ("Drag a handle of the frame", "Move that edge or corner"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+Shift+Z, Ctrl+Y", "Redo"),
    ("Ctrl+O", "Open a folder"),
    ("Ctrl+E", "Show or hide the export window"),
    ("F1", "Show or hide this list"),
];

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut open = app.help_open;
    egui::Window::new("Shortcuts")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts")
                .num_columns(2)
                .striped(true)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    for (keys, action) in SHORTCUTS {
                        ui.strong(*keys);
                        ui.label(*action);
                        ui.end_row();
                    }
                });
        });
    app.help_open = open;
}
