---
name: egui-app
description: Structure and patterns for the pagecrop desktop window built with eframe and egui - panels, textures, background loading, drawing a rotated image with a crop frame, pointer and keyboard input. Use for any work in crates/app.
---

# The egui app

egui redraws the whole window from scratch on every frame. There are no stored widgets. `update()` reads
the state, draws it, and changes the state when the user does something.

Function names below are from the versions known when this was written. If one does not compile, look
it up in the docs of the installed version. Do not guess.

## Files

| File | Holds |
|---|---|
| `main.rs` | `fn main`, reads the optional folder argument, calls `eframe::run_native`. |
| `app.rs` | `struct App` (all state) and `impl eframe::App`. `update()` only calls the panel functions. |
| `panels/top.rs`, `pages.rs`, `adjust.rs`, `status.rs` | One function per panel: `pub fn show(app: &mut App, ctx: &egui::Context)`. |
| `editor.rs` | The central canvas: picture, crop frame, pointer handling. |
| `loader.rs` | Worker threads and channels for thumbnails and previews. |
| `export_dialog.rs` | The export window and its progress. |
| `theme.rs` | Colours, spacing, fonts. |

Keep `update()` short. A panel function longer than about 80 lines should be split.

## Window skeleton

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.poll_workers(ctx);                 // take finished images from channels
    self.handle_shortcuts(ctx);
    panels::top::show(self, ctx);           // TopBottomPanel::top
    panels::status::show(self, ctx);        // TopBottomPanel::bottom
    panels::pages::show(self, ctx);         // SidePanel::left
    panels::adjust::show(self, ctx);        // SidePanel::right
    editor::show(self, ctx);                // CentralPanel, must come last
}
```

Side and top panels must be added before the `CentralPanel`.

## State changes go through one place

Every edit of the current page uses the same three steps, so undo and saving stay correct:

1. When the pointer goes down or a slider starts moving, remember `before = project.entries[i].key`.
2. While it moves, call `project.set_key(i, new_adjust)` every frame. The screen follows live.
3. When it is released, push the edit to the history and save the sidecar file.

Put this in helper methods on `App` (`begin_edit`, `apply_edit`, `commit_edit`). Panels call the helpers
and never touch `project.entries` directly.

## Background work

The UI thread must never decode a file. Pattern:

```rust
// worker side
let image = decode_and_shrink(&path);      // slow
let _ = tx.send(Loaded { index, generation, image });
ctx.request_repaint();                     // wake the UI

// UI side, at the top of update()
while let Ok(msg) = self.rx.try_recv() {
    if msg.generation == self.generation { self.store(ctx, msg); }
}
```

- `egui::Context` is cheap to clone and may be sent to another thread.
- `generation` is a counter that goes up each time a folder is opened. Results from an older folder
  are dropped.
- Previews use one dedicated thread that always works on the newest request. Thumbnails use their own
  small rayon pool. Thumbnails must never delay the preview of the page the user is looking at.

## Textures

```rust
let color = egui::ColorImage::from_rgb([w as usize, h as usize], rgb.as_raw());
let tex: egui::TextureHandle = ctx.load_texture("preview", color, egui::TextureOptions::LINEAR);
```

Keep the `TextureHandle` in the state. Dropping it frees the texture. Create a texture when the picture
arrives, never once per frame.

## Drawing the rotated picture

```rust
let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
let painter = ui.painter_at(rect);                       // clips to the canvas

let size = egui::vec2(w0 * zoom, h0 * zoom);
let mut mesh = egui::Mesh::with_texture(tex.id());
mesh.add_rect_with_uv(
    egui::Rect::from_center_size(center, size),
    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
    egui::Color32::WHITE,
);
mesh.rotate(egui::emath::Rot2::from_angle(total_radians), center);
painter.add(egui::Shape::mesh(mesh));
```

A positive angle must turn the picture clockwise. Check this by eye the first time. The crop frame is
drawn afterwards with plain upright rectangles; see the `image-geometry` skill for the conversion
between screen positions and frame fractions.

## Pointer input on the canvas

```rust
if response.drag_started() { /* hit-test at response.interact_pointer_pos(), remember handle and start crop */ }
if response.dragged()      { /* total delta = current pos - start pos; core::crop_edit::drag(...) */ }
if response.drag_stopped() { /* commit_edit */ }
if let Some(pos) = response.hover_pos() { /* set cursor icon from hit-test */ }
```

Compute drags from the start position and the start crop, not by adding per-frame deltas. Adding deltas
drifts.

## Keyboard

Read keys with `ctx.input(|i| i.key_pressed(egui::Key::ArrowRight))`. Skip shortcuts while a text field
has focus: `if ctx.wants_keyboard_input() { return; }`.

## Sliders

```rust
let r = ui.add(egui::Slider::new(&mut value, -15.0..=15.0).step_by(0.05).suffix("°"));
if r.drag_started() { app.begin_edit(); }
if r.changed()      { app.apply_edit(...); }
if r.drag_stopped() || (r.changed() && !r.dragged()) { app.commit_edit(); }
```

## Long lists

Use `egui::ScrollArea::vertical().show_rows(ui, row_height, count, |ui, range| { ... })` so only the
visible rows are drawn. All rows must have the same height.

## Things that go wrong

| Symptom | Cause |
|---|---|
| Window freezes while opening a folder | Decoding on the UI thread. |
| Picture appears only after moving the mouse | The worker did not call `ctx.request_repaint()`. |
| Memory grows with every page | Textures or previews are created per frame or never evicted. |
| Everything is slow in `cargo run` | The dev profile settings from row 0001.1 are missing. |
| Two widgets react to the same click | Two widgets share an id; wrap repeated rows in `ui.push_id(index, ...)`. |
