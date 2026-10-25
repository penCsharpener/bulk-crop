## Phase 0005 — Settings file per version, four-cornered frame (P2)

Two requests after phase 0004:

1. **Each version keeps its own settings file.** pagecrop is not at 1.0 yet, and the settings format
   will keep changing. Instead of loading every old format forever, each minor version reads and
   writes its own file, `pagecrop_v05.json` for 0.5. A user who wants the settings of an older
   version copies the old file to the new name. Whatever still fits is used; whatever cannot be read
   is dropped, named in a notice, and has to be set again.
2. **Straighten a page photographed at an angle.** A page shot from slightly above or below comes out
   as a trapezoid, not a rectangle, and turning it does not help. Holding **Ctrl** and dragging a
   corner of the frame moves that corner alone, so the frame can follow the four corners of the page.
   The export maps the four-sided frame back onto a rectangle (a perspective correction).

Read `CLAUDE.md` and `docs/Roadmaps/readme.md` first, and the skill `image-geometry`. This roadmap
changes rules in both: row 0005.1 updates `CLAUDE.md`, row 0005.2 updates the skill.

## The model and decisions

### Settings file per version

- `persist::SIDECAR_NAME` keeps its name and becomes
  `concat!("pagecrop_v", env!("CARGO_PKG_VERSION_MAJOR"), env!("CARGO_PKG_VERSION_MINOR"), ".json")`.
  The workspace version is shared by all crates, so it is the tool's version. Patch releases share a
  file; 0.5.x uses `pagecrop_v05.json`, 0.6.x `pagecrop_v06.json`, 1.0.x `pagecrop_v10.json`.
- Row 0005.1 bumps the workspace version to **0.5.0**, so this phase writes `pagecrop_v05.json` from
  the start.
- There is **no migration and no fallback**. A folder that has only `pagecrop.json` (0.4 and earlier)
  or another version's file opens with default settings. The older file is never read, changed or
  deleted by the app.
- The `version` field of the file goes away; the file name is the version.
- **Lenient loading**, so a copied file gives as much as it can:
  - Text that is not JSON, or JSON that is not an object, is still `CoreError::Sidecar` (the app keeps
    it as `<name>.bak`, as today).
  - Otherwise each part is read on its own with `serde_json::from_value`: `sort`, `export` and every
    entry of `keys`. A part that fails is left out and its name goes into `Saved::dropped`: the
    page's file name for a key, `sort` or `export` for those. `keys` that is not an object drops as
    `keys`. A missing part is not dropped; it takes its default. Unknown fields are ignored.
  - Inside one key, `#[serde(default)]` fills missing fields. A field with a wrong type drops the
    whole key.
- `Project` gets `pub dropped: Vec<String>`, filled from `Saved::dropped` in `Project::build` (empty
  when there was no file), so the app can say what was left out.
- `persist::older_settings(dir: &Path) -> Vec<String>`: the names of files in `dir` that start with
  `pagecrop`, end with `.json`, are not `SIDECAR_NAME` and are files, sorted. A folder that cannot be
  read gives an empty list; this is only a hint.
- The app shows notices in the status bar (`App::notice`), one at a time, in this order of priority:
  1. the existing "unreadable, kept as … .bak" notice;
  2. when `project.dropped` is not empty: `Some settings in pagecrop_v05.json could not be read and
     were left out: page_003.jpg, export` (at most five names, then `and N more`);
  3. when the folder has no `SIDECAR_NAME` file and `older_settings` is not empty: `This folder has
     settings from another pagecrop version (pagecrop.json). To use them, copy the file to
     pagecrop_v05.json and open the folder again.` (the first older name only).
  The file names in the texts come from `SIDECAR_NAME` and the lists, never typed out.
- The command line's `scan` prints the same two hints to stderr, without the length limit.

### The four-cornered frame

The frame stays a rectangle `{cx, cy, w, h}` as before, and each of its four corners may be shifted
away from the rectangle's corner. Everything that already works on the rectangle (moving, dragging an
edge, fitting, the spin handle) keeps working; the shifts ride along.

```rust
/// A crop frame in fractions of the picture: the centre (`cx`, `cy`) and the size (`w`, `h`) of a
/// rectangle, and how far each of its corners is shifted from the rectangle's corner, in the order
/// NW, NE, SE, SW (clockwise on screen), x then y, in fractions of the picture like `cx` and `cy`.
/// The default is the whole picture with no shift.
pub struct CropRect {
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
    pub corner_shift: [(f32, f32); 4],
}
```

In the file a frame looks like
`"crop": {"cx": 0.0, "cy": 0.0, "w": 0.7, "h": 0.8, "corner_shift": [[0.07, 0.0], [-0.07, 0.0], [0.0, 0.0], [0.0, 0.0]]}`.
A frame without `corner_shift` (a copied 0.4 file) loads as a rectangle.

The corner order NW, NE, SE, SW is used everywhere in this phase: in `corner_shift`, in
`frame_corners`, in `SquareToQuad::new`, and for the screen points of the frame. `Handle::NW`,
`Handle::NE`, `Handle::SE`, `Handle::SW` are indexes 0, 1, 2, 3.

**Geometry functions** (`geometry.rs`):

```rust
/// The four corners of the frame, NW, NE, SE, SW, in fractions of the base picture from its centre:
/// the rectangle's corners plus `corner_shift`.
pub fn frame_corners(crop: CropRect) -> [(f32, f32); 4] {
    let (hw, hh) = (crop.w / 2.0, crop.h / 2.0);
    let signs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut out = [(0.0, 0.0); 4];
    for ((o, (sx, sy)), (dx, dy)) in out.iter_mut().zip(signs).zip(crop.corner_shift) {
        *o = (crop.cx + sx * hw + dx, crop.cy + sy * hh + dy);
    }
    out
}

/// Whether the frame's four corners make a shape the export can straighten: every corner stays in
/// its own quarter around the rectangle's centre (NW up and left of it, and so on), and the shape is
/// strictly convex. Non-finite values are not valid.
pub fn quad_ok(crop: CropRect) -> bool
```

`quad_ok` rules, with `p = frame_corners(crop)` and the centre `(crop.cx, crop.cy)`:
- NW: `x < cx && y < cy`; NE: `x > cx && y < cy`; SE: `x > cx && y > cy`; SW: `x < cx && y > cy`.
- For each corner `i`, with `a = p[i] - p[i-1]` and `b = p[i+1] - p[i]` (indexes wrap),
  `a.x * b.y - a.y * b.x > 0`. On screen (y down) the order NW, NE, SE, SW turns clockwise, so all four
  cross products of a convex frame are positive. Computing this in fractions instead of pixels does
  not change the signs.
- Both rules are needed: a frame with every corner in its own quarter can still be dented (test
  `dented_frame_is_not_valid` below).

**The perspective map** (`geometry.rs`, after Heckbert, "Fundamentals of Texture Mapping", 1989):

```rust
/// The projective map from the unit square onto a four-sided shape: (0, 0) goes to the first corner,
/// (1, 0) to the second, (1, 1) to the third and (0, 1) to the fourth, so NW, NE, SE, SW. Straight
/// lines stay straight. For a rectangle or a parallelogram it is the plain linear map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquareToQuad {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
}

impl SquareToQuad {
    /// The map onto `corners`, given NW, NE, SE, SW in any unit. A shape with three corners on one
    /// line has no such map; it gets the linear map instead, and `quad_ok` keeps such frames out of
    /// the settings.
    pub fn new(corners: [(f32, f32); 4]) -> SquareToQuad {
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = corners;
        let (sx, sy) = (x0 - x1 + x2 - x3, y0 - y1 + y2 - y3);
        let (dx1, dy1) = (x1 - x2, y1 - y2);
        let (dx2, dy2) = (x3 - x2, y3 - y2);
        let den = dx1 * dy2 - dx2 * dy1;
        let (g, h) = if den == 0.0 {
            (0.0, 0.0)
        } else {
            ((sx * dy2 - dx2 * sy) / den, (dx1 * sy - sx * dy1) / den)
        };
        SquareToQuad {
            a: x1 - x0 + g * x1,
            b: x3 - x0 + h * x3,
            c: x0,
            d: y1 - y0 + g * y1,
            e: y3 - y0 + h * y3,
            f: y0,
            g,
            h,
        }
    }

    /// Where the point (`s`, `t`) of the unit square lands.
    pub fn apply(&self, s: f32, t: f32) -> (f32, f32) {
        let w = self.g * s + self.h * t + 1.0;
        (
            (self.a * s + self.b * t + self.c) / w,
            (self.d * s + self.e * t + self.f) / w,
        )
    }
}
```

Worked values (checked): corners (−2, −3), (2, −3), (4, 3), (−4, 3). The four unit corners land on
them, (0.5, 0.5) lands on (0, −1), where the diagonals cross, and (0.5, 0), (0.5, 0.5), (0.5, 1)
land on (0, −3), (0, −1), (0, 3), one straight line. For the rectangle (−4, −3), (4, −3), (4, 3),
(−4, 3), the point (0.25, 0.75) lands on (−2, 1.5).

**What changes in existing functions:**

- `frame_inside` checks the four `frame_corners` with `point_inside`, instead of the rectangle's
  corners. Because the frame is convex and the allowed region is convex, corners inside means the
  whole frame is inside.
- `fit_inside` still shrinks about `(cx, cy)` by one factor `s` and never grows. `s` now scales
  `w`, `h` **and** `corner_shift`, which keeps `quad_ok`. Both limits are worked per corner, with
  `v` the corner's offset from the centre (`frame_corners` minus the centre): the upright rectangle
  needs `cx + s·v.x` and `cy + s·v.y` within `-0.5..=0.5`; the turned picture is unchanged in form,
  with `v` taken from `frame_corners` instead of the rectangle. The private `CORNERS` constant goes.
- `output_size` measures the frame in pixels: the width is the mean of the top edge (NW to NE) and the
  bottom edge (SW to SE), the height the mean of the left edge (NW to SW) and the right edge (NE to
  SE), each length `hypot(dx · wb, dy · hb)`. Then rounding and `Resize` as before. With no shift this
  is `w · wb` and `h · hb`, as today. This is the usual estimate of the page's real proportions; see
  Risks.
- `quarter_turn` turns the shifts with the frame. Clockwise, every corner moves one place on
  (NW → NE → SE → SW → NW) and each shift `(dx, dy)` becomes `(−dy, dx)`; so the new NE shift is the
  old NW shift turned, and so on. Counter-clockwise, each corner moves one place back and `(dx, dy)`
  becomes `(dy, −dx)`. `turn_crop` uses `quarter_turn`, so it follows.
- `Adjust::normalized`: after the existing size floor, a frame for which `quad_ok` is false gets
  `corner_shift` all zero.
- `render::rotate_crop`: the output pixel `(u, v)` becomes `s = (u + 0.5) / ow`, `t = (v + 0.5) / oh`,
  then `SquareToQuad::new(frame corners in base pixels).apply(s, t)` gives the frame point
  `(px, py)`; `frame_to_src` and `sample` are unchanged. The frame corners in base pixels are
  `frame_corners(crop)` with x times `wb` and y times `hb`. With no shift this is the old formula, so
  every existing render test stays green unchanged.

### Editing the four corners

- **Ctrl + drag on a corner handle** moves that corner alone: its shift becomes the shift when the
  drag began plus the pointer's movement in frame fractions. Ctrl is read when the drag starts
  (`modifiers.command`, as the other shortcuts do); letting go of Ctrl during the drag changes
  nothing until the next drag. Ctrl + drag anywhere else is a plain drag.
- Alt was the first idea, but Xfce, MATE, Cinnamon and other Linux desktops move the whole window on
  Alt + drag, so the app would never see it.
- The moved corner stays inside the turned picture (`frame_inside`) and the frame stays valid
  (`quad_ok`). Where the movement would break either, the corner goes as far as it can and then
  slides along the limit on each axis alone, exactly as `drag_inside` does for the rectangle.
- Plain drags are unchanged: Move moves the rectangle, an edge moves the rectangle's edge, a corner
  handle resizes the rectangle. The shifts are kept, so the shape moves and stretches with it. A plain
  drag also stops where the frame would no longer be valid. The rectangle itself may reach past the
  picture by as much as the corners are shifted (row 0005.4 fix): with the corners pulled in from a
  full-picture frame, Move must still move the shape. `frame_inside` limits the real corners.
- `hit_test` works on the four real corners:
  1. Corners first, in the order NW, NE, SW, SE as today: the pointer is within `tol` of a corner
     point on both axes.
  2. Then edges, in the order W, E, N, S as today. The N edge runs from NW to NE: the pointer's x is
     within the two corners' x range widened by `tx`, and its y is within `ty` of the edge's y at that
     x (linear between the two corners; when their x are equal, the mean of their y). S is the same
     from SW to SE. W runs from NW to SW with the axes swapped: y within the corners' y range widened
     by `ty`, x within `tx` of the edge's x at that y. E is the same from NE to SE.
  3. Then Move when the pointer is inside the frame or on its outline: all four cross products of
     (edge, pointer − edge start) in the order NW, NE, SE, SW are `>= 0`.
  With no shift this gives exactly today's answers, so the existing tests stay green.
- The editor draws the real shape: the outline through the four corners, the shade outside it, the
  corner handles at the corners, the edge handles at the middle of each edge, and the grid through
  `SquareToQuad` so its lines meet where the page's lines will be straight.
- The spin handles sit beside the bounding box of the four corners.
- **"Square corners"** (button in the Rotation section, below "0°") sets all shifts to zero, then fits
  the frame with `fit_inside` (the rectangle can reach further out than the shifted corners did). It
  is greyed out while the frame has no shift. One click is one undo step.
- Alternating sides, flowing to the following pages, undo and redo need nothing new: the shifts are
  part of `crop`.
- The editor shows the picture as it is, not warped. The **P** result view already renders with
  `render`, so it shows the straightened page.

## Tasks

| # | Task | Depends on | Done when |
|---|------|-----------|-----------|
| ✅ 0005.1 | **Settings file per version** (core + command line + docs). No skill. Read "Settings file per version".<br>Bump `version` to `0.5.0` in the root `Cargo.toml` and in the `pagecrop-core`/`pagecrop-testkit` dependency lines of `crates/app/Cargo.toml` and `crates/cli/Cargo.toml`; commit the updated `Cargo.lock`.<br>`persist.rs`: `SIDECAR_NAME` as described; remove `version` from `File` (and its `Default`); `Saved` gets `pub dropped: Vec<String>`; `load` reads leniently as described; `pub fn older_settings(dir: &Path) -> Vec<String>`. `serde_json::Value` is already available through `serde_json`.<br>`model.rs`: `Project` gets `pub dropped: Vec<String>`, set in `build`.<br>CLI `scan`: the two hints on stderr, after the list. Its doc comment names the settings file through the constant's rule, not `pagecrop.json`.<br>Tests in `persist.rs`: `settings_file_name_carries_the_version` (`SIDECAR_NAME == format!("pagecrop_v{}{}.json", env!("CARGO_PKG_VERSION_MAJOR"), env!("CARGO_PKG_VERSION_MINOR"))` and it is not `pagecrop.json`); `unreadable_key_is_dropped_and_named` (keys `page_001.jpg` with brightness 5 and `page_002.jpg` with `"brightness": "loud"`: keys hold only page 1, `dropped == ["page_002.jpg"]`); `wrong_sort_falls_back_to_name` (`"sort": "sideways"`: sort is Name, `dropped == ["sort"]`); `wrong_export_is_dropped` (`"export": 7`: export None, `dropped == ["export"]`); `keys_that_are_not_an_object_drop_as_keys`; `json_that_is_not_an_object_is_sidecar_error` (`[1, 2]`); `old_file_without_version_name_is_not_read` (only a `pagecrop.json` with a key: `load` gives None); `older_settings_lists_other_versions` (folder with `pagecrop.json`, `pagecrop_v04.json`, `SIDECAR_NAME`, `pagecrop.json.bak`, `notes.json` and a directory `pagecrop_x.json`: the result is `["pagecrop.json", "pagecrop_v04.json"]`); `older_settings_of_missing_folder_is_empty`. The existing persist tests stay; those that write JSON by hand keep working because the extra `version` field is ignored.<br>Test in `model.rs`: `dropped_parts_reach_the_project`.<br>Docs: `CLAUDE.md` "Data that must not be lost": the app writes "the settings file `persist::SIDECAR_NAME` (`pagecrop_v05.json` in 0.5)" into the photo folder, and the next bullet says the same file. `CLAUDE.md` "Types and signatures": the serde bullet ends "…so a settings file copied from an older version loads what still fits. Each minor version has its own settings file; there is no migration." `README.md`: the section "The settings file" is renamed and rewritten for the versioned name, the copy rule, what is dropped, and without `version`; the sentence "pagecrop writes only …" names the versioned file. `docs/performance.md`: the table row names the versioned file. | — | `cargo test --workspace` is green with the ten new tests. `cargo run -p pagecrop-cli -- scan samples/book` on a copy of a folder that has only `pagecrop.json` lists no `*` pages and prints the copy hint; after `cp pagecrop.json pagecrop_v05.json` the `*` marks are back. A `pagecrop_v05.json` with one key edited to `"contrast": "x"` prints that page's name as left out and still marks the others. |
| 0005.1f | **Notices for dropped and older settings** (screen). Skill: egui-app.<br>`app.rs`, `open_folder`: after a successful open, set `self.notice` as described under "Settings file per version", in that priority. Put the text into a pure function `fn settings_notice(name: &str, dropped: &[String], has_current: bool, older: &[String]) -> Option<String>` next to `keep_unreadable_settings`. `keep_unreadable_settings` builds its message from `SIDECAR_NAME` instead of the typed `pagecrop.json.bak`, and its doc comment says the same.<br>Tests in `app.rs`: `no_notice_for_a_clean_folder`; `dropped_parts_are_named` (two names); `long_drop_list_is_cut_after_five` (seven names: five, then "and 2 more"); `older_file_hint_only_without_current_file`; `dropped_wins_over_older_hint`. | 0005.1 | `cargo test -p pagecrop` is green with the five tests. Open a copy of `samples/book` that has only a `pagecrop.json`: the status bar says the folder has settings from another version and names both files. Copy it to `pagecrop_v05.json` and open the folder again: the settings are back and there is no notice. Break one key's type: the notice names that page; the other pages keep their settings. |
| ✅ 0005.2 | **Four-cornered frame in the model and geometry** (core). Skill: image-geometry. Read "The four-cornered frame".<br>`model.rs`: `corner_shift` on `CropRect` (default all zero; the struct already has `#[serde(default)]`); `normalized` as described.<br>`geometry.rs`: `frame_corners`, `quad_ok`, `SquareToQuad` exactly as above; `frame_inside`, `fit_inside`, `output_size` and `quarter_turn` as described. Remove `CORNERS`.<br>Every `CropRect { cx, cy, w, h }` literal in the workspace (code and tests, including `crop_edit::drag`, the CLI's `parse_crop` and the test helpers) either sets `corner_shift: [(0.0, 0.0); 4]` or uses `..CropRect::default()` / `..start`. In `crop_edit::drag` the result keeps `start.corner_shift`; no other change to `crop_edit.rs` in this row.<br>Tests in `geometry.rs`: `frame_corners_of_plain_frame_are_its_corners`; `frame_corners_add_the_shift`; `rectangle_is_valid_quad`; `crossed_corners_are_not_valid` (frame (0, 0, 0.6, 0.6), NW shift (0.7, 0)); `dented_frame_is_not_valid` (frame (0, 0, 0.6, 0.6), shifts NE (0, 0.28), SE (−0.28, −0.28), SW (0.28, 0): every corner is in its own quarter, but SE dents the shape); `nan_shift_is_not_valid`; `square_to_quad_hits_the_corners`, `square_to_quad_centre_is_where_diagonals_cross` and `square_to_quad_keeps_lines_straight` with the worked values; `square_to_quad_of_rectangle_is_linear` ((0.25, 0.75) → (−2, 1.5)); `fit_scales_the_shift_too` (frame (0.1, 0, 0.9, 0.9) with NW shift (0.05, 0.02) at 6°: fitted shift equals start shift times `fitted.w / start.w`, and the fitted frame is inside); `shifted_frame_outside_is_not_inside` (full frame with NW shift (−0.05, 0) at 0°); `output_size_of_trapezoid` (base 1000 × 1000, frame (0, 0, 0.8, 0.6), NW shift (0.1, 0), NE shift (−0.1, 0): top 600 px, bottom 800, sides `hypot(100, 600)` ≈ 608.3, so `Resize::None` gives (700, 608)); `quarter_turn_moves_shifts_round` (clockwise: the old NW shift (0.01, 0.02) is the new NE shift (−0.02, 0.01)); `four_clockwise_turns_return_to_start` and `clockwise_then_counter_is_identity` gain a shift on every corner (extend the existing tests' start frames).<br>Tests in `model.rs`: `invalid_shift_is_cleared_by_normalized` (crossed corners → all zero, rectangle unchanged); `valid_shift_survives_normalized`.<br>Test in `persist.rs`: `frame_without_corner_shift_loads_as_rectangle`; `corner_shift_round_trips`.<br>Skill `image-geometry`: section "The crop frame" says the frame is a rectangle plus `corner_shift` and names `frame_corners`; "From an output pixel to the source" gives the `SquareToQuad` step and says it reduces to the old formula without shift; "Pressing a 90° button" gives the shift rule; "Frame stays inside" names `quad_ok`. | — | `cargo test --workspace` is green with the eighteen new tests, and every existing test passes unchanged except the literal fixes. The app still runs on `samples/book` and behaves as before. |
| ✅ 0005.3 | **Straightening render, keystoned test pages** (core + testkit + command line). Skills: image-geometry, image-io.<br>`render.rs`: `rotate_crop` through `SquareToQuad` as described.<br>Testkit: `pub fn keystone_image(w: u32, h: u32, index: u32, squeeze: f32) -> RgbImage`: one page on the table like `page_image` (same colours, text lines and markers), photographed from below: the page is 0.8·h high and centred; its bottom edge is 0.7·w wide and its top edge `0.7·w·(1 − squeeze)`, the sides straight. For a pixel, `v = (y + 0.5 − 0.1·h) / (0.8·h)`, the half width at that row `0.35·w·(1 − squeeze·(1 − v))`, `u = (x + 0.5 − w/2) / (2 · half) + 0.5`; outside `0..=1` on either is table. `pub fn write_keystone_book(dir: &Path, count: u32, w: u32, h: u32, squeeze: f32) -> std::io::Result<Vec<PathBuf>>`: like `write_book` with every page from `keystone_image`.<br>CLI: `render` gains `--corners nwx,nwy,nex,ney,sex,sey,swx,swy` (eight numbers, default all zero, parsed like `--crop`; a frame for which `quad_ok` is false is an error naming the option). `gen-fixtures` gains `--keystone <squeeze>`, conflicting with `--drift`, `--reverse-mtime` and `--two-sided`, which calls `write_keystone_book`.<br>Test in testkit: `keystone_top_is_narrower` (squeeze 0.2 on 800 × 600: the pixel at (0.5·w − 0.33·w, 0.12·h) is table, the one at (0.5·w − 0.33·w, 0.88·h) is page).<br>Tests in `render.rs`: `keystoned_page_fills_the_frame` (in memory: `keystone_image(800, 600, 0, 0.2)`, frame (0, 0, 0.66, 0.76) with NW shift (0.068, 0) and NE shift (−0.068, 0); every pixel of the first and last row and column of `render_rgb(…, Resize::None)` is page colour, red channel > 200; the same frame without shift has a top corner of table colour, red < 100); `shifted_render_has_output_size` (the render's dimensions equal `output_size`); `no_shift_render_is_unchanged` (a gradient rendered with a fine angle and a crop gives the same pixels as with `corner_shift` explicitly zero — a guard for the refactor). | 0005.2 | `cargo test --workspace` is green with the four new tests. `cargo run -p pagecrop-cli -- gen-fixtures samples/keystone --keystone 0.2 --count 6` writes six trapezoid pages. `cargo run -p pagecrop-cli -- render samples/keystone/page_001.jpg --out samples/keystone-straight.jpg --crop 0,0,0.66,0.76 --corners 0.068,0,-0.068,0,0,0,0,0` writes a picture whose page fills it, with straight, parallel edges and level text lines (look at it). Crossed corners are rejected with a clear message. |
| ✅ 0005.4 | **Hit test and corner drag on four corners** (core). Skill: image-geometry. Read "Editing the four corners".<br>`crop_edit.rs`: `hit_test` on the real corners as described (same signature). `drag_inside` also requires `quad_ok` in its `ok`. Move the "go as far as it can, then slide on each axis" part of `drag_inside` into a private `fn reach_towards(ok: impl Fn((f32, f32)) -> bool, delta: (f32, f32)) -> (f32, f32)` and use it in both. New `pub fn drag_corner(start: CropRect, handle: Handle, delta: (f32, f32), base: Size, angle_deg: f32) -> CropRect`: `start` is fitted first like in `drag_inside`; for NW, NE, SE, SW the shift of that corner becomes the start shift plus `delta`, kept to `frame_inside` and `quad_ok` with `reach_towards`; any other handle returns the fitted start unchanged.<br>Tests: every existing `crop_edit` test stays green unchanged except literal fixes. New: `hit_test_finds_a_shifted_corner` (NW shifted (0.1, 0.05): the old rectangle corner is no longer NW, the shifted point is); `hit_test_follows_a_slanted_edge` (N edge from a shifted NW to NE: a point on the slanted line is N, a point on the old straight top line far from it is not); `inside_of_a_trapezoid_is_move`; `corner_drag_moves_one_corner` (the other three shifts and `cx, cy, w, h` are unchanged); `corner_drag_stops_at_the_picture_edge` (full-size frame at 0°, dragging NW outwards leaves it at the edge, `frame_inside` holds); `corner_drag_never_dents_the_frame` (dragging SE far inwards past the NE–SW diagonal stops while `quad_ok` still holds); `corner_drag_slides_along_the_limit`; `plain_drag_keeps_the_shift` (a W drag of a shifted frame keeps `corner_shift`); `plain_drag_stops_before_crossing_corners` (a frame with large inward NW shift: shrinking it from the W edge stops while `quad_ok` holds); `edge_handle_in_drag_corner_changes_nothing`. | 0005.2 | `cargo test -p pagecrop-core crop_edit` is green with the ten new tests and the existing ones. |
| ✅ 0005.4f | **Ctrl + drag in the editor** (screen). Skills: egui-app, image-geometry. Read "Editing the four corners".<br>`app.rs`: `Drag` gains `pub corner: bool`; a corner drag calls `drag_corner`, any other `drag_inside` as today.<br>`editor.rs`: `Layout::frame_points(crop) -> [egui::Pos2; 4]` (NW, NE, SE, SW on screen from `frame_corners`); `frame_rect` becomes the bounding box of those points. `paint_frame` draws: the shade as one `egui::Mesh` ring between the frame and a rectangle that contains both `area` and the frame (two triangles per side: outer i, outer i+1, inner i+1 and outer i, inner i+1, inner i, with both rings in the order NW, NE, SE, SW), the outline as a `closed_line` (dark 2.5, then white 1.5 on top), the grid lines from `SquareToQuad` built on the screen points (line `k` from `apply(k/8, 0)` to `apply(k/8, 1)`, and the same across), the corner handles at the points and the edge handles at the middle of each edge (`handle_points` takes the four points). `handle_pointer`: on press, Ctrl held (`modifiers.command`) and a corner handle hit start a corner drag. The cursor is `Crosshair` while Ctrl is held over a corner and during a corner drag; otherwise as today.<br>`panels/adjust.rs`: the "Square corners" button as described, with the hover text "Ctrl + drag a corner of the frame to follow a page photographed at an angle; the export straightens it. This makes the frame a rectangle again."<br>`help.rs`: `("Ctrl + drag a frame corner", "Move that corner alone, for a page shot at an angle")`, after the line for dragging a handle. `README.md`: the framing step of the workflow gets two sentences on Ctrl + drag and Square corners, and the shortcuts table the new line. | 0005.3, 0005.4 | `cargo run -p pagecrop-cli -- gen-fixtures samples/keystone --keystone 0.2`, then `cargo run -p pagecrop -- samples/keystone`. Ctrl + drag the NW and the NE handle onto the page's top corners: the frame becomes a trapezoid, the shade follows its slanted edges, the grid lines lean with the page's sides. P shows a straight page with no table at its edges. Pages 2–12 show the same frame. A plain drag on the W edge moves that slanted edge; Move moves the trapezoid; the spin handle turns it and it stays inside. Ctrl + drag beyond the photo stops at its edge; dragging a corner across the frame stops before the shape folds. Ctrl+Z undoes one corner drag at a time. ⟳ 90° keeps the corners on the same page corners. Square corners gives a rectangle, and Ctrl+Z gives the trapezoid back. Restart: the trapezoid is still there, and `pagecrop_v05.json` holds `corner_shift`. Export: the pictures are straight pages. |
| 0005.5 | **Final check on a keystoned two-sided book** (no code unless a bug turns up; a bug fix gets its own commit `0005.5: fix …`).<br>Make `samples/final5` with `gen-fixtures --keystone 0.15 --count 20`. In the app: set a trapezoid on page 1; turn on alternating on page 1 and give page 2 a different trapezoid; turn page 9 by 90° and back; brighten +20; delete one photo; export. Then copy a 0.4-style `pagecrop.json` (write one by hand with two keys, one of them with a wrong type) into a fresh copy of the folder as `pagecrop_v05.json` and open it.<br>Write the findings, including anything awkward, as a short list at the end of this file under "## Results of 0005.5". | 0005.1f, 0005.4f | All exports exist, none shows table colour at its edges, odd and even pages carry their own trapezoid. The copied file gives the readable key, the notice names the other, and nothing in the photo folder changed except `pagecrop_v05.json` and the one photo in the trash. |

## Risks

- **Settings of 0.4 do not open by themselves.** A user who updates and opens an old folder sees the
  default frame on every page until they copy the file. The notice of 0005.1f says how. This is the
  price of not keeping old formats while the tool is below 1.0.
- **The output proportions are an estimate.** Four corners do not fix the page's real width to height
  ratio; that needs the camera's focal length. The mean of opposite edges is what scanner apps use and
  is close for the small angles of a hand-held photo of a book. A strongly tilted shot comes out a
  little squashed along the tilt.
- **Page curl is not fixed.** The correction is for a flat page. Lines that bend towards the spine
  stay bent.
- **The far end of a tilted page is resampled up.** The narrow edge of the trapezoid is stretched to
  the mean width, so its text is softer than the near end. That is in the photo; no setting fixes it.
- **The shade ring assumes a tame frame.** Its triangles are correct while each corner lies in its own
  quarter, which `quad_ok` guarantees.
- **Ctrl may be taken by other tools.** Some screen readers and remote-desktop tools use Ctrl + drag.
  None of the common Linux desktops does.

## Not in this phase

- Finding the page corners automatically.
- Typing a known page size or ratio for the output.
- A migration tool for older settings files.
