## Phase 0001 — Crop, straighten and shrink book-page photos in bulk (P1)

Photographing a book page by page produces a few hundred JPEG files in which the book drifts: it turns
a little and slides a little with every page. Fixing each photo by hand is the same small correction
repeated hundreds of times. The old Windows tools (JPEGCrops, jpegcrop) crop losslessly but cannot turn
a picture by two degrees and cannot shrink it, so they do not fit this job.

The end state is a Linux desktop program, working name **pagecrop**, written in Rust with egui. It opens
a folder, shows every photo, and lets the user set a crop frame, a rotation and tone on one page. That
adjustment applies to every following page until the next page that has its own adjustment. Going back
to page 10 and changing it therefore affects pages 10 to 29 when page 30 already has its own. Nothing is
written until the user starts the export; then every page is rotated, cropped, tone-adjusted, shrunk and
saved once into a separate folder.

Decided and not open for discussion in this roadmap: desktop only, Linux only, egui, no browser build,
no lossless (jpegtran-style) path. Each exported file is decoded once and encoded once.

Two things in this program are valuable and must not be lost: the original photos, which are never
written to, and the adjustments, which live in a small file `pagecrop.json` next to the photos.

This plan is written so that a small model can follow it. Each row is one sitting, names its files and
functions, and ends in something that can be run. Read `CLAUDE.md` and `docs/Roadmaps/readme.md` first.

## Before the first row

- Rust is installed with rustup (stable channel, with `rustfmt` and `clippy`).
- A C linker is present (`build-essential` on Debian).
- eframe and the file dialog may need system packages. If the linker names a missing library, install
  its `-dev` package and continue. Likely candidates on Debian: `pkg-config`, `libxkbcommon-dev`,
  `libwayland-dev`, `libx11-dev`, `libgtk-3-dev`. This list is a guess, not a tested fact.
- Optional but useful: put five real phone photos, at least one taken in portrait, into
  `samples/real/`. They are the only way to check EXIF handling (see Risks).

## Skills this work needs

| Skill | What it covers | Where it is written down |
|---|---|---|
| Working a row | Order of steps, gates, when to stop and ask | `.claude/skills/roadmap-row` |
| Rust basics for this repo | Errors, borrowing, casts, tests, dependencies | `CLAUDE.md` |
| Image geometry | Frame units, rotation direction, pixel mapping, output size | `.claude/skills/image-geometry` |
| Image files | Decode, EXIF orientation and date, shrink, tone, encode, test pictures | `.claude/skills/image-io` |
| egui | Panels, textures, worker threads, drawing, input | `.claude/skills/egui-app` |

## The model and decisions

### Crates

```
crates/testkit  pagecrop-testkit   synthetic pages            depends on: image
crates/core     pagecrop-core      all logic, no UI           depends on: image, exif, serde, rayon ...
crates/cli      pagecrop-cli       terminal commands          depends on: core, testkit, clap, anyhow
crates/app      pagecrop           the egui window            depends on: core, eframe, rfd, anyhow
```

The command-line tool exists so that every core row can be run and checked from a terminal before its
screen exists. It is not a product feature.

### Core modules (`crates/core/src`)

| Module | Introduced in | Public items |
|---|---|---|
| `error.rs` | 0001.3 | `CoreError` |
| `scan.rs` | 0001.3 | `list_jpegs` |
| `model.rs` | 0001.3, grows in .5 and .6 | `Project`, `ImageEntry`, `SortBy`, `Adjust`, `CropRect` |
| `decode.rs` | 0001.4 | `read_orientation`, `apply_orientation`, `load_oriented`, `shrink_to_long_edge`, `read_taken` |
| `geometry.rs` | 0001.7 | `base_size`, `frame_to_src`, `half_extents`, `output_size`, `quarter_turn`, `Resize` |
| `crop_edit.rs` | 0001.7b | `Handle`, `hit_test`, `drag` |
| `render.rs` | 0001.8, grows in .9 | `rotate_crop`, `render_rgb`, `render` |
| `encode.rs` | 0001.8 | `write_jpeg` |
| `tone.rs` | 0001.9 | `lut`, `apply`, `to_gray` |
| `persist.rs` | 0001.10 | `save`, `load`, `SIDECAR_NAME` |
| `export.rs` | 0001.11 | `ExportOptions`, `ExportJob`, `Progress`, `plan`, `export_one`, `run` |
| `history.rs` | 0001.12 | `Edit`, `History` |

### Types

```rust
pub struct CropRect { pub cx: f32, pub cy: f32, pub w: f32, pub h: f32 }   // fractions, see image-geometry
// Default: { cx: 0.0, cy: 0.0, w: 1.0, h: 1.0 }  = the whole picture

pub struct Adjust {
    pub quarter_turns: u8,   // 0..=3, each 90° clockwise
    pub angle_deg: f32,      // -45.0..=45.0, positive = clockwise on screen
    pub crop: CropRect,
    pub brightness: i32,     // -100..=100
    pub contrast: i32,       // -100..=100
    pub grayscale: bool,
}
// Default: everything zero / false, crop = whole picture

pub struct ImageEntry {
    pub file_name: String,
    pub path: PathBuf,
    pub taken: Option<i64>,  // seconds; EXIF date, else file modification time   (from 0001.5)
    pub key: Option<Adjust>, // Some = this page has its own settings             (from 0001.6)
}

pub enum SortBy { Name, Date }

pub struct Project {
    pub dir: PathBuf,
    pub entries: Vec<ImageEntry>,                 // in display order
    pub sort: SortBy,                             // from 0001.5
    pub orphans: BTreeMap<String, Adjust>,        // settings for files not found   (from 0001.10)
    pub export: ExportOptions,                    // from 0001.11
}

pub enum Resize { None, LongEdge(u32), Percent(f32) }

pub struct ExportOptions { pub out_dir: PathBuf, pub resize: Resize, pub quality: u8, pub overwrite: bool }
// Default: out_dir = "<dir>/cropped", resize = LongEdge(1600), quality = 90, overwrite = false
```

All of these derive `Debug, Clone, PartialEq`; `CropRect`, `Adjust`, `SortBy`, `Resize` also `Copy`;
everything stored in `pagecrop.json` also `Serialize, Deserialize` with `#[serde(default)]`.

### How settings flow forward

Each page either has its own settings (`key = Some`) or it has none.

```
page:      1   2   3   4   5   6   7   8   9   10  11  12
key:       -   -   -   A   -   -   -   B   -   -   -   -
effective: d   d   d   A   A   A   A   B   B   B   B   B          d = default
```

- `effective(i)` is the key of the nearest page at or before `i` that has one, else the default.
- Editing page `i` always does `set_key(i, changed copy of effective(i))`.
- Setting a key on page 6 in the picture above changes pages 6 and 7 only. Page 8 keeps `B`.
- `clear_key(i)` removes the page's own settings; it falls back to the page before it.

Nothing else is needed for "apply to all following pages": it is how reading works, not a copy step.

Page numbers in this document and on screen start at 1. Indices in code start at 0, so "page 4" is
`entries[3]`. Test names use page numbers.

### The picture pipeline

```
file --decode--> RGB --EXIF orientation--> source S
S --rotate by k*90 + a, cut the frame (bilinear)--> mid image (frame size in source pixels)
mid --Lanczos shrink--> output size --greyscale?--> --tone table--> JPEG
```

The editor draws the same thing cheaply: a small preview of S as a rotated texture with the frame on
top. Row 0001.8f adds a "Show result" view that runs the real pipeline on the preview, so the two can be
compared by eye.

### Why not the other way

- **Store the frame in pixels.** Fractions make one frame fit the full photo, the preview and photos of
  another size, with no conversion.
- **Copy settings to following pages on every edit.** Hundreds of writes per mouse move, and "stop at
  the next adjusted page" would need extra bookkeeping. Looking backwards for the nearest key gives the
  rule for free.
- **Rotate and shrink in one resampling pass.** Sharper in theory, but it needs an area filter that is
  easy to get wrong. Two well-known steps are easier to test. See Risks.
- **Settings in a central app folder.** A file next to the photos moves with them and survives
  reinstalling.

## Tasks

| # | Task | Depends on | Done when |
|---|------|-----------|-----------|
| ✅ 0001.1 | **Workspace and command-line stub** (core + command line).<br>Run `git init`. Create a Cargo workspace (`resolver = "3"`, edition 2024) with members `crates/core`, `crates/cli`, `crates/testkit`. Add `rust-toolchain.toml` (channel `stable`, components `rustfmt`, `clippy`). Add `.gitignore` with `target/` and `samples/`.<br>In the root `Cargo.toml` add `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 3`, plus `[profile.dev.package.pagecrop-core] opt-level = 3` and the same for `pagecrop-testkit`.<br>`pagecrop-core`: `pub fn version() -> &'static str` returning `env!("CARGO_PKG_VERSION")`, with one test `version_is_not_empty`.<br>`pagecrop-cli`: uses `clap` (derive) with subcommands; for now only `--version`. | — | `cargo run -p pagecrop-cli -- --version` prints `pagecrop-cli 0.1.0`. The three gates from `CLAUDE.md` finish without a warning. `git status` shows no `target/` files. |
| ✅ 0001.1f | **Empty window** (screen). Skill: egui-app.<br>Add `crates/app` (`pagecrop`) with `eframe`. `main.rs` opens a 1280×800 window titled `pagecrop`, minimum 900×600, dark visuals. `app.rs` holds `struct App` and `update()`, which calls four panel functions in `panels/`: top bar with the text "pagecrop", bottom status bar with "No folder open", left panel 220 wide headed "Pages", right panel 260 wide headed "Adjust", and a central area with the centred text "Open a folder to begin". | 0001.1 | `cargo run -p pagecrop` opens a window showing the four labelled areas and the centred text. Resizing the window keeps the side panels at their widths. Closing it ends the process with exit code 0. |
| ✅ 0001.2 | **Synthetic book** (core + command line). Skill: image-io.<br>`pagecrop-testkit`: `pub fn page_image(w: u32, h: u32, index: u32, angle_deg: f32) -> RgbImage`. Background `(40,40,40)`. A page rectangle `0.70*w` by `0.80*h`, centred, turned clockwise by `angle_deg`, colour `(245,245,240)`. For each pixel, turn it back by `-angle_deg` around the centre and test whether it lies inside the rectangle. Inside, with page-local `u`,`v` from 0 to 1: dark `(30,30,30)` "text lines" where `0.1 < u < 0.9`, `0.15 < v < 0.9` and `((v*40.0) as u32) % 2 == 1`; and `index + 1` dark squares in the band `0.04 < v < 0.09`, each `0.03` wide, starting at `u = 0.1` and stepping `0.05`.<br>`pub fn write_book(dir: &Path, count: u32, w: u32, h: u32, drift_deg: f32) -> std::io::Result<Vec<PathBuf>>` writes `page_001.jpg` … at quality 90, page `i` (from 0) tilted by `i * drift_deg`.<br>Command: `gen-fixtures <dir> [--count 12] [--width 1600] [--height 1200] [--drift 0.4]`.<br>Tests in testkit: `page_centre_is_page_colour`, `corner_is_background`, `write_book_creates_count_files` (tempdir, 3 files of 80×60). | 0001.1 | `cargo run -p pagecrop-cli -- gen-fixtures samples/book` creates 12 files `page_001.jpg` to `page_012.jpg`. Opened in an image viewer, each shows a light page with dark lines on a dark table, page 1 level and page 12 visibly tilted clockwise, and page 3 has three squares at its top. |
| ✅ 0001.3 | **List a folder** (core + command line). Skill: image-io.<br>`error.rs`: `CoreError` with `Io { path, source }` for now. `scan.rs`: `pub fn list_jpegs(dir: &Path) -> Result<Vec<PathBuf>, CoreError>`, rules from the image-io skill, sorted with `natord::compare_ignore_case` on the file name.<br>`model.rs`: `ImageEntry { file_name, path }`, `Project { dir, entries }`, `pub fn Project::open(dir: &Path) -> Result<Project, CoreError>`. `Project::open` is the only place that builds a `Project`.<br>Command: `scan <dir>` prints one file name per line, then `N images`.<br>Tests: `lists_only_jpegs_in_natural_order` (tempdir with `b10.jpg`, `b2.JPG`, `a.jpeg`, `notes.txt`, `.hidden.jpg`, `x.jpg.tmp`, folder `sub/` → `a.jpeg, b2.JPG, b10.jpg`), `missing_dir_is_error_with_path`. | 0001.2 | `cargo run -p pagecrop-cli -- scan samples/book` prints the 12 names in order and `12 images`. `scan /no/such/dir` prints an error that contains `/no/such/dir` and exits with code 1. |
| ✅ 0001.3f | **Open a folder and walk the list** (screen). Skill: egui-app.<br>`App` gains `project: Option<Project>`, `selected: usize`, `generation: u64`, `error: Option<String>`. `main.rs` takes an optional folder path as first argument and opens it at start. Top bar: button "Open folder…" using `rfd::FileDialog::new().pick_folder()`. Left panel: one row per entry showing the file name, drawn with `ScrollArea::show_rows`; clicking a row selects it; the selected row is highlighted and scrolled into view. Keys: Down / PageDown / Right select the next page, Up / PageUp / Left the previous, Home and End the first and last. Status bar: `3 / 12 · page_003.jpg`. An error from `Project::open` is shown in red in the status bar; the previous folder stays open. | 0001.3 | `cargo run -p pagecrop -- samples/book` lists 12 names with page 1 highlighted. Pressing Down four times shows `5 / 12 · page_005.jpg`. End jumps to 12, Down does nothing more. "Open folder…" on an empty folder shows `0 / 0` and no crash; on a folder without read permission the status bar shows the error and the list stays as it was. |
| ✅ 0001.4 | **Decode and orient** (core + command line). Skill: image-io.<br>`decode.rs`: `read_orientation(path) -> u8` (1 on any problem), `apply_orientation(img: RgbImage, o: u8) -> RgbImage` using the table in the skill, `load_oriented(path) -> Result<RgbImage, CoreError>`, `shrink_to_long_edge(img: &RgbImage, max: u32) -> RgbImage` using `imageops::thumbnail` and never enlarging. Add `CoreError::Decode { path, message }`.<br>Command: `info <file>` prints `WxH orientation=N`, where W×H is the size after orienting.<br>Tests: `orientation_1_to_8_moves_pixels_correctly` on a 3×2 image whose six pixels have six different colours, checking size and the colour at `(0,0)` for each of the eight values; `shrink_1600x1200_to_160_gives_160x120`; `shrink_never_enlarges`; `empty_file_is_decode_error`. | 0001.3 | `cargo run -p pagecrop-cli -- info samples/book/page_001.jpg` prints `1600x1200 orientation=1`. `info` on a text file renamed to `.jpg` prints a decode error naming the file. If `samples/real/` holds a portrait phone photo, `info` prints a size that is taller than wide. |
| ✅ 0001.4f1 | **Show the selected page** (screen). Skill: egui-app.<br>`loader.rs`: a preview worker thread. It receives `PreviewRequest { index, generation, path }` over a channel, always skips to the newest waiting request, calls `load_oriented` then `shrink_to_long_edge(.., 1600)`, and sends `Loaded { index, generation, image: RgbImage, full_size: (u32, u32) }` back, then calls `ctx.request_repaint()`. A decode error is sent back as a message too.<br>`App` requests a preview whenever `selected` or the folder changes. `editor.rs` draws the preview texture centred and fitted into the central area with a 4 % margin. While waiting it shows "Loading…"; on error it shows the message. Status bar gains the full size: `3 / 12 · page_003.jpg · 1600×1200`. | 0001.4 | With `samples/book` open, each page appears within about half a second of selecting it. Holding Down for two seconds never freezes the window and ends on the page the status bar names. A broken JPEG placed in the folder shows its error text in the centre while the other pages still work. |
| ✅ 0001.4f2 | **Thumbnails in the page list** (screen). Skill: egui-app.<br>`loader.rs`: a thumbnail pool (own rayon pool, threads = cores − 1, at least 1). On opening a folder, one job per entry: `load_oriented`, `shrink_to_long_edge(.., 160)`, send back with `generation`. `App` keeps `thumbs: Vec<Option<TextureHandle>>`. Left panel rows become 96 high: thumbnail fitted into 120×84, file name under it, a grey box while the thumbnail is missing. Results with an old `generation` are dropped. | 0001.4f1 | Opening `samples/book` fills in 12 thumbnails within a few seconds while the window stays responsive. Selecting a page during that time still shows its large preview promptly. Opening a second folder before the first has finished shows only thumbnails of the second folder. |
| ✅ 0001.5 | **Sort by date** (core + command line). Skill: image-io.<br>`decode.rs`: `parse_exif_datetime(s: &str) -> Option<i64>` and `read_taken(path) -> Option<i64>` (EXIF `DateTimeOriginal`, else file modification time).<br>`model.rs`: `ImageEntry.taken`, `SortBy`, `Project.sort`, `pub fn sort(&mut self, by: SortBy)`: by name as before; by date ascending with missing dates last and ties broken by natural name. `Project::open` fills `taken` and sorts by name.<br>testkit: `write_book` gets a parameter `reverse_mtime: bool` that sets file times with `filetime` so that the last page is the oldest. Command: `gen-fixtures --reverse-mtime`, and `scan <dir> [--sort name / date]` printing the date before each name.<br>Tests: `parses_exif_datetime`, `rejects_garbage_datetime`, `date_sort_follows_mtime`, `name_sort_restores_order`, `ties_fall_back_to_name`. | 0001.4 | After `gen-fixtures samples/book-rev --reverse-mtime`, `scan samples/book-rev --sort date` lists `page_012.jpg` first and `page_001.jpg` last; `--sort name` lists them 1 to 12. |
| ✅ 0001.5f | **Sort switch** (screen).<br>Top bar: two selectable labels "Name" and "Date". Choosing one calls `Project::sort`, reorders `thumbs` to match, and keeps the same file selected (find it again by `file_name`). | 0001.5 | With `samples/book-rev` open and page 3 selected, clicking "Date" reverses the list, the highlight stays on `page_003.jpg` (now row 10) and thumbnails still match their names. Clicking "Name" restores the order. |
| ✅ 0001.6 | **Settings that flow forward** (core). Read "How settings flow forward" above.<br>`model.rs`: `CropRect`, `Adjust` (all fields from the Types section, with `Default`), `ImageEntry.key`. Methods on `Project`: `effective(&self, i: usize) -> Adjust`; `key_index(&self, i: usize) -> Option<usize>` (the page whose key page `i` uses); `set_key(&mut self, i: usize, a: Adjust)` (stores `a.normalized()`); `clear_key(&mut self, i: usize)`; `span(&self, i: usize) -> Range<usize>` (from `i` to the next page with a key, or the end). `Adjust::normalized(self) -> Adjust` clamps `angle_deg` to ±45, `quarter_turns` to `% 4`, brightness and contrast to ±100, and crop `w`,`h` to at least 0.02. Out-of-range `i` returns the default and changes nothing.<br>Tests, on 12 entries: `no_keys_gives_default_everywhere`; `key_on_4_applies_to_4_through_12`; `second_key_on_8_ends_first_span`; `key_on_6_between_two_keys_stops_before_8`; `clear_key_falls_back_to_previous`; `span_of_4_is_4_to_8`; `normalized_clamps_all_fields`; `out_of_range_index_is_harmless`. | 0001.5 | `cargo test -p pagecrop-core model` runs the eight named tests green. The test `key_on_6_between_two_keys_stops_before_8` asserts in particular that pages 8 to 12 still return the key set on page 8. |
| ✅ 0001.6f | **Rotate, and see it flow forward** (screen). Skills: egui-app, image-geometry.<br>Add `App` helpers `begin_edit`, `apply_edit(adjust)`, `commit_edit` as described in the egui-app skill (history and saving are empty stubs for now).<br>Right panel, section "Rotation": buttons "⟲ 90°" and "⟳ 90°" (change `quarter_turns` only, for now), slider "Fine angle" −15 to 15 in steps of 0.05 with a number field, button "0°". Section "Page": the text "Own settings" or "Follows page N", and a button "Reset page" that calls `clear_key` and is disabled when the page has no key.<br>Editor: draw the preview as a mesh turned by `quarter_turns * 90 + angle_deg` around its centre; fit the base size, not the turned outline, so the picture does not jump while the slider moves.<br>Page list: a small accent-coloured dot on rows that have their own key.<br>Sort switch: if any page has a key, ask "Sort order decides which pages follow which. Switch anyway?" before sorting. | 0001.6 | In `samples/book`: set 2° on page 4 → pages 4 to 12 show the page turned clockwise by 2°, pages 1 to 3 are unchanged, row 4 has a dot. Set −1° on page 8 → pages 8 to 12 follow it, 4 to 7 still show 2°. Go to page 5, set 3° → pages 5 to 7 show 3°, page 8 still −1°. "Reset page" on 5 → pages 5 to 7 show 2° again and the dot is gone. The "Page" section reads "Follows page 4" on page 6. |
| 0001.7 | **Geometry functions** (core). Skill: image-geometry — every formula is there.<br>`geometry.rs`: `Size { w: f32, h: f32 }`; `base_size(src: Size, quarter_turns: u8) -> Size`; `frame_to_src(p: (f32, f32), src: Size, quarter_turns: u8, angle_deg: f32) -> (f32, f32)`; `half_extents(base: Size, angle_deg: f32) -> (f32, f32)`; `Resize`; `output_size(base: Size, crop: CropRect, resize: Resize) -> (u32, u32)`; `quarter_turn(a: Adjust, clockwise: bool) -> Adjust` (changes `quarter_turns` and the crop together).<br>Tests: `worked_example_1_quarter_turn`, `worked_example_2_identity` (numbers from the skill); `four_clockwise_turns_return_to_start`; `clockwise_then_counter_is_identity`; `half_extents_at_zero_are_half`; `output_size_none_long_edge_percent` (base 1600×1200, crop w 0.5 h 1.0 → 800×1200; LongEdge(600) → 400×600; Percent(25) → 200×300); `long_edge_never_enlarges`. | 0001.6 | `cargo test -p pagecrop-core geometry` runs the seven named tests green, and `worked_example_1_quarter_turn` asserts that frame point (−150, −200) of a 400×300 source turned once maps to source position (0, 300). |
| 0001.7b | **Frame dragging logic** (core). Skill: image-geometry.<br>`crop_edit.rs`: `enum Handle { Move, N, S, E, W, NE, NW, SE, SW }`; `hit_test(crop: CropRect, pos: (f32, f32), tol: (f32, f32)) -> Option<Handle>` — corners first, then edges, then inside = `Move`, else `None`; all positions are frame fractions. `drag(start: CropRect, handle: Handle, delta: (f32, f32), half: (f32, f32), min: (f32, f32)) -> CropRect` — works on the four edges; `Move` shifts all four and clamps so the frame stays within ±`half`; an edge or corner handle moves only its edges, clamped to ±`half` and to `min` away from the opposite edge. `start` is the frame when the drag began and `delta` the total movement since then.<br>Tests: `hit_corner_before_edge`, `hit_inside_is_move`, `hit_far_outside_is_none`, `move_is_clamped_to_bounds`, `east_handle_changes_only_right_edge`, `north_west_handle_changes_top_and_left`, `cannot_shrink_below_min`, `cannot_drag_edge_past_bounds`. | 0001.7 | `cargo test -p pagecrop-core crop_edit` runs the eight named tests green; `east_handle_changes_only_right_edge` asserts that left, top and bottom edges are unchanged after the drag. |
| 0001.7f | **Crop frame in the editor** (screen). Skills: egui-app, image-geometry.<br>Editor draws, over the picture: four half-transparent black rectangles covering everything outside the frame, a 1.5 px white outline with a 1 px dark outline around it, and eight 9×9 handle squares. Pointer: convert the position to frame fractions, `hit_test` with a tolerance of 8 screen points, set the cursor icon (resize arrows or move), and on drag call `begin_edit` / `drag` / `apply_edit` / `commit_edit`. Bounds come from `half_extents`, minimum size 0.02.<br>The 90° buttons now call `geometry::quarter_turn`, so the frame turns with the picture.<br>Right panel, section "View": checkbox "Grid" drawing 7 evenly spaced horizontal and 7 vertical thin lines inside the frame, as a reference for level text. Status bar gains the frame size in source pixels: `· frame 1120×960`. | 0001.7b | In `samples/book` page 1: dragging the right-middle handle left narrows the frame and darkens the area it leaves; dragging inside moves the frame; the frame cannot leave the picture nor shrink to nothing. Page 2 shows the same frame, and row 1 has a dot. Pressing "⟳ 90°" turns picture and frame together so the same part of the page stays framed. With "Grid" on and the fine angle set so the text lines of page 6 are parallel to the grid, the slider reads about −2°. |
| 0001.8 | **Render one picture** (core + command line). Skills: image-geometry, image-io.<br>`render.rs`: `rotate_crop(src: &RgbImage, adj: &Adjust, out_w: u32, out_h: u32) -> RgbImage` — for every output pixel compute the source position with the formula in the skill and sample bilinearly, white outside. `render_rgb(src: &RgbImage, adj: &Adjust, resize: Resize) -> RgbImage` — size of the mid image from `output_size(.., Resize::None)`, `rotate_crop`, then a Lanczos3 resize if the final size differs.<br>`encode.rs`: `write_jpeg(path: &Path, img: &DynamicImage, quality: u8) -> Result<(), CoreError>` via `<name>.tmp` and rename.<br>Command: `render <file> --out <file> [--turns N] [--angle DEG] [--crop cx,cy,w,h] [--long-edge N] [--percent P] [--quality 90]`. Negative numbers must be accepted (`allow_negative_numbers`).<br>Tests: `identity_returns_identical_pixels`; `one_turn_equals_imageops_rotate90`; `right_half_crop_equals_crop_imm`; `outside_source_is_white`; `straightened_page_fills_frame` — `page_image(800, 600, 0, 4.0)` rendered with angle −4.0 and crop `{0, 0, 0.66, 0.76}` is 528×456 and all four corner pixels have red above 200, while the same crop at angle 0 has at least one corner with red below 100; `long_edge_resize_gives_expected_size`. | 0001.7 | `cargo run -p pagecrop-cli -- render samples/book/page_010.jpg --angle -3.6 --crop 0,0,0.66,0.76 --long-edge 800 --out /tmp/page.jpg` writes an 800×691 picture in which the page fills the whole picture, its text lines run level and no dark table is visible. No `/tmp/page.jpg.tmp` is left behind. |
| 0001.8f | **Show result** (screen).<br>Right panel "View": checkbox "Show result" (key `P`). When on, the editor shows `render_rgb(preview, effective, LongEdge(longer side of the canvas))` instead of the picture with frame; no frame, no handles. Cache the texture and recompute only when the page or its effective settings change. | 0001.8 | On page 6 of `samples/book`, with a frame around the page and the angle set to level: "Show result" displays exactly the framed area, upright, and the same part of the page that the frame enclosed, to within a couple of screen points on every side. Turning the fine angle by +5° in the frame view and switching again shows the result tilted the same way the picture was. |
| 0001.9 | **Tone** (core + command line). Skill: image-io.<br>`tone.rs`: `lut(brightness: i32, contrast: i32) -> [u8; 256]`, `apply(img: &mut RgbImage, table: &[u8; 256])`, `to_gray(img: &RgbImage) -> GrayImage`.<br>`render.rs`: `render(src: &RgbImage, adj: &Adjust, resize: Resize) -> DynamicImage` = `render_rgb`, then greyscale if set, then the table unless both values are 0. Returns `ImageLuma8` for greyscale, else `ImageRgb8`. `render_rgb` keeps its signature.<br>Command `render` gains `--brightness N --contrast N --gray` and now calls `render`.<br>Tests: `zero_zero_is_identity_table`; `brightness_100_maps_128_to_255`; `contrast_minus_100_flattens_to_mid_grey` (every entry 127 or 128); `table_is_monotonic_for_positive_contrast`; `gray_of_pure_green_is_150`; `render_gray_returns_luma8`. | 0001.8 | `render samples/book/page_001.jpg --gray --contrast 40 --out /tmp/g.jpg` writes a picture that an image viewer reports as greyscale, with a visibly whiter page and blacker lines than the original. |
| 0001.9f | **Tone controls** (screen).<br>Right panel, section "Tone": sliders "Brightness" and "Contrast" (−100 to 100, whole numbers), checkbox "Greyscale", button "Reset tone". They edit the current page through the edit helpers, so they flow forward like angle and frame.<br>The editor shows tone live: keep the preview's `RgbImage`, apply greyscale and the table to a copy, upload that as the texture; redo only when the page or the three tone values change. | 0001.9 | On page 4 of `samples/book`, checking "Greyscale" and setting contrast to 40 changes the picture at once; pages 5 to 12 show the same tone, pages 1 to 3 stay in colour. "Show result" shows the same tone. Dragging a slider quickly stays smooth. |
| 0001.10 | **Remember the settings** (core + command line).<br>`persist.rs`: `SIDECAR_NAME = "pagecrop.json"`. File content: `{ "version": 1, "sort": "name", "keys": { "<file name>": Adjust, ... } }`. `save(project: &Project) -> Result<(), CoreError>` writes all keys of `entries` plus all `orphans`, to `pagecrop.json.tmp`, then renames. `Saved { sort: SortBy, keys: BTreeMap<String, Adjust> }`. `load(dir: &Path) -> Result<Option<Saved>, CoreError>` returns `None` when the file is absent and `CoreError::Sidecar { path, message }` when it cannot be parsed.<br>`Project::open` calls `load`, applies the saved sort, attaches each key to the entry with that file name, and puts keys whose file is missing into `orphans`. A parse error is returned to the caller.<br>Command `scan` prints `*` in front of names that have their own key.<br>Tests: `save_then_load_round_trips_keys_and_sort`; `key_for_missing_file_becomes_orphan_and_is_saved_again`; `absent_file_is_none`; `garbage_file_is_sidecar_error`; `no_tmp_file_left_after_save`; `file_with_unknown_extra_field_still_loads`. | 0001.9 | After writing a `pagecrop.json` by hand with a key for `page_004.jpg`, `scan samples/book` shows `*` only in front of `page_004.jpg`. Replacing the file content with `{` makes `scan` print an error naming `pagecrop.json`. |
| 0001.10f | **Autosave and reload** (screen).<br>`commit_edit`, "Reset page" and the sort switch call `persist::save`. Status bar shows "Saved" for two seconds, or stays on "Could not save: <reason>" in orange when saving fails; the app keeps working either way.<br>When `Project::open` fails with a `Sidecar` error: copy the file to `pagecrop.json.bak`, show "Settings file was unreadable, kept as pagecrop.json.bak", and open the folder with no keys. | 0001.10 | Set an angle on page 4 and a frame on page 8, close the window, start `pagecrop samples/book` again: both pages have their dots and their settings. After `chmod a-w samples/book`, an edit shows "Could not save" and the app keeps working (`chmod u+w` afterwards). With the file replaced by `{`, the app opens the folder, shows the message, and `pagecrop.json.bak` holds the broken text. |
| 0001.11 | **Export** (core + command line). Skill: image-io.<br>`export.rs`: `ExportOptions` (Types section); `ExportJob { index, src, dst, adjust }`; `enum Outcome { Written, Skipped, Failed(String) }`; `enum Progress { Started { total }, Item { index, outcome }, Finished { written, skipped, failed, cancelled } }`.<br>`plan(project: &Project, opts: &ExportOptions) -> Result<Vec<ExportJob>, CoreError>`: error `OutDirIsSource` if the canonical output folder equals the photo folder; creates the output folder; one job per entry with `effective(i)` and `dst = out_dir / file_name`.<br>`export_one(job: &ExportJob, opts: &ExportOptions) -> Outcome`: skip if `dst` exists and `overwrite` is false; else `load_oriented`, `render`, `write_jpeg`.<br>`run(jobs: &[ExportJob], opts: &ExportOptions, cancel: &AtomicBool, on_progress: &(dyn Fn(Progress) + Sync))`: a rayon pool with `min(cores, 8)` threads; checks `cancel` before each job.<br>`Project` gains `export`, stored in `pagecrop.json` under `"export"`.<br>Command: `export <dir> --out <dir> [--long-edge N / --percent P / --no-resize] [--quality 90] [--overwrite]`, one line per file, a summary line, exit code 2 if anything failed.<br>Tests (tempdir, six 400×300 fixtures, key on page 1 default, key on page 4 with crop `{0, 0, 0.5, 1.0}`, LongEdge(200)): `writes_six_files_with_expected_sizes` (pages 1–3 are 200×150, pages 4–6 are 133×200); `out_dir_equal_to_source_is_refused`; `existing_file_is_skipped_without_overwrite`; `existing_file_is_replaced_with_overwrite`; `cancel_before_start_writes_nothing`; `broken_file_fails_and_others_succeed`; `no_tmp_files_remain`; `source_files_are_byte_identical_afterwards`. | 0001.10 | `cargo run --release -p pagecrop-cli -- export samples/book --out /tmp/out --long-edge 800` prints 12 lines and `12 written, 0 skipped, 0 failed`; `/tmp/out` holds 12 pictures at most 800 pixels on the long side, each showing what the settings in `pagecrop.json` describe. Running it again prints `0 written, 12 skipped`. `export samples/book --out samples/book` refuses with a message and writes nothing. |
| 0001.11f | **Export window** (screen). Skill: egui-app.<br>Top bar button "Export…" (Ctrl+E) opens a window: output folder (text field plus "Choose…"), resize choice "Keep size" / "Long edge" with a pixel field / "Percent" with a number field, quality slider 60 to 100, checkbox "Replace existing files", and a line "N pages, M already exist in the output folder". Button "Start" runs `plan` and then `run` on a background thread; progress arrives over a channel. While running: a progress bar, "37 / 212", and "Cancel". At the end: "N written, N skipped, N failed" and a scrollable list of failed files with their reasons. Options are saved to `pagecrop.json` when the export starts. Editing is disabled while an export runs.<br>Status bar now shows the output size of the current page: `· frame 1120×960 → 800×686`. | 0001.11 | Exporting `samples/book` to `samples/book/cropped` with "Long edge 800" shows the bar advance to 12 / 12 and "12 written"; the files exist and match "Show result" for the same pages. Starting again shows "12 already exist" and, without "Replace existing files", ends with "12 skipped". Pressing "Cancel" during an export of a large real folder stops within a second or two and leaves no `.tmp` files. Choosing the photo folder itself as output shows the refusal message. |
| 0001.12 | **Undo history** (core).<br>`history.rs`: `Edit { index: usize, before: Option<Adjust>, after: Option<Adjust> }`; `History` with `push(edit)` (clears redo; ignores edits where before equals after), `undo(&mut self, p: &mut Project) -> Option<usize>`, `redo(..) -> Option<usize>` (both return the page index they touched), `can_undo`, `can_redo`, `clear`. At most 200 entries; the oldest is dropped.<br>Tests: `undo_restores_previous_key`; `undo_of_first_key_removes_it`; `redo_reapplies`; `push_after_undo_clears_redo`; `no_op_edit_is_not_recorded`; `history_is_capped_at_200`. | 0001.11 | `cargo test -p pagecrop-core history` runs the six named tests green; `undo_of_first_key_removes_it` asserts that the page follows its predecessor again afterwards. |
| 0001.12f | **Undo and redo** (screen).<br>`commit_edit` and "Reset page" push an `Edit`. Ctrl+Z undoes, Ctrl+Shift+Z and Ctrl+Y redo; two buttons "↶" "↷" in the top bar, disabled when there is nothing to do. After undo or redo the affected page is selected and the file is saved. Opening another folder clears the history. | 0001.12 | Drag the frame on page 3, set an angle on page 7, press Ctrl+Z twice: page 7 is selected and level again, then page 3 is selected and its frame is back and its dot gone. Ctrl+Shift+Z twice brings both back. Restarting the app shows the state on screen at the moment it was closed. |
| 0001.13 | **Zoom and pan** (screen). Skill: image-geometry, section "Screen and frame space".<br>Editor state `zoom_factor` (1 = fit, up to 8) and `pan`. Mouse wheel zooms around the pointer; dragging with the middle button, or with the left button while Space is held, pans; key `0` or a double click on empty canvas returns to fit. Handles keep their size on screen and the 8-point tolerance stays in screen points. Changing the page keeps zoom and pan, so the same spot of consecutive pages can be compared. | 0001.7f | In `samples/book`, zooming in on the top-left corner of the page keeps the point under the pointer in place; handles are still grabbable and move the frame by the distance the pointer moves; stepping to the next page shows the same corner; `0` returns to the fitted view. |
| 0001.14 | **Shortcuts and help** (screen).<br>Add: `[` and `]` change the fine angle by 0.1° (with Shift by 1°); `R` and Shift+R turn 90° clockwise and counter-clockwise; `G` grid; `P` show result; `N` and Shift+N jump to the next and previous page with its own settings; Delete resets the page; Ctrl+O opens a folder. Each goes through the edit helpers. Key `F1` or a "?" button opens a window listing every shortcut in a two-column table. Shortcuts do nothing while a text field has focus. | 0001.12f | With the keyboard alone: open `samples/book`, press `N` to reach the first adjusted page, `]` five times to add 0.5°, Right to move on, `P` to check the result, Ctrl+Z to take the change back. Typing `0` into the long-edge field of the export window does not reset the zoom. `F1` lists all shortcuts named in this roadmap. |
| 0001.15 | **Preview cache and read-ahead** (screen).<br>`App` keeps the last 12 previews (texture plus `RgbImage`) in least-recently-used order. The preview request becomes an ordered list: selected page, then the two after it, then the one before; the worker takes the first one not yet cached. Tone and result textures are rebuilt from the cache without decoding. | 0001.9f | Stepping forward through a folder of at least 30 real photos at one page per second never shows "Loading…" after the first page. Stepping back two pages is instant. Memory use of the process stays flat after the first 20 pages (watch it in a system monitor). |
| 0001.16 | **Look and feel** (screen).<br>`theme.rs`: one function setting a flat dark style — near-black panels, one accent colour for selection, dots and the frame handles, 6 px rounding, 8 px spacing, slightly larger text. Section headings in the right panel as small caps or dim labels. Selected row in the page list gets an accent bar on its left edge. Window icon. Panels are resizable within sensible limits (left 160–360, right 240–360). | 0001.14 | A screenshot of the app with `samples/book` open shows: no default-grey egui widgets, the selected page clearly marked, the frame handles in the accent colour, and all text readable at 100 % scale on this machine. Nothing overlaps at the minimum window size of 900×600. |
| 0001.17 | **Measure export speed** (command line only, no code change expected).<br>Run the release build on a folder of about 100 real photos: `time cargo run --release -p pagecrop-cli -- export <folder> --out /tmp/speed --long-edge 1600 --overwrite`. Write `docs/performance.md` with: machine, number and size of photos, total time, photos per second, peak memory (`/usr/bin/time -v`). | 0001.11 | `docs/performance.md` exists with the five numbers filled in from a real run, and states whether the result is above or below 5 photos per second (the signal named in Risks). |
| 0001.18 | **Install and document** (screen + docs).<br>`README.md`: what the program does, how to build and install (`cargo install --path crates/app`), the workflow in six steps, the shortcut table, where `pagecrop.json` lives and what it holds, the list of deferred features. `assets/pagecrop.desktop` with name, command and icon, and one paragraph in the README on copying it to `~/.local/share/applications`. | 0001.16 | After `cargo install --path crates/app`, typing `pagecrop ~/some/photo/folder` in a new terminal from any directory opens that folder. A person who has not seen the program can export a cropped book by following only the README. |
| 0001.19 | **Final check on a real book** (no code).<br>Take a folder of at least 50 real photos of a book. Before starting: `sha256sum *.jpg > /tmp/before.txt`. Work through the book in the app: adjust page 1, then only where the drift becomes visible; go back once and change an earlier page; undo once; export with a long edge of 1600. Afterwards: `sha256sum *.jpg > /tmp/after.txt` and `diff /tmp/before.txt /tmp/after.txt`. | 0001.18 | `diff` prints nothing, so no original was touched. The output folder holds one picture per photo. Paging through the output in an image viewer, every page is upright and framed as it was in the app. The only new entries in the photo folder are `pagecrop.json` and the export folder. The number of pages that needed their own settings is written into `docs/performance.md`. |

## Deferred

Not in this roadmap. Each can be added later without changing what is built here.

- Lossless crop for pages that need neither a fine angle nor shrinking.
- Running in a browser.
- Two crop frames per photo for double-page spreads.
- Four-corner perspective correction.
- Windows and macOS builds.
- Copying EXIF data or setting the print resolution in the output.
- Leaving single photos out of the export.
- Thumbnails that show the cropped result instead of the whole photo.
- Finding the page edges and the angle automatically.

## Risks worth naming before starting

- **Preview and export can disagree about direction.** The editor turns a texture with egui; the export
  turns pixels with our own formula. One flipped sign gives an export tilted the opposite way, and unit
  tests of either side alone will not notice. Row 0001.8f exists to compare the two by eye, and
  `straightened_page_fills_frame` pins the export side. Do not skip the visual check in 0001.6f that a
  positive angle turns clockwise.

- **No test picture carries EXIF data.** The synthetic pages have no orientation tag and no date tag,
  so reading those tags is checked only by hand with real photos in `samples/real/`. If that folder is
  empty, rows 0001.4 and 0001.5 can be marked done while portrait photos still arrive lying on their
  side. The final row 0001.19 would catch it, late. Put real photos there before 0001.4.

- **Function names for egui, eframe, rfd, image and exif are written from memory.** These crates change
  between versions; `Mesh::rotate`, `add_rect_with_uv`, the closure passed to `eframe::run_native` and
  the EXIF reader calls are the likeliest to differ. The rule in `CLAUDE.md` applies: read the docs of
  the installed version, never guess, never downgrade. A small model that invents a name and then
  "fixes" the compiler error by removing the feature is the failure to watch for.

- **Without the dev-profile settings from 0001.1 the app looks broken.** Unoptimised image code is ten
  to thirty times slower. A preview then takes seconds, and every screen row after 0001.4f1 seems to
  fail its "Done when". If the app feels slow in `cargo run`, check the root `Cargo.toml` first.

- **The file dialog may not build or open on this machine.** `rfd` needs either GTK development files
  or a running desktop portal. The app also accepts the folder as a command-line argument, so every row
  can still be checked. If the dialog cannot be made to work, replace it with the `egui-file-dialog`
  crate, which needs nothing from the system.

- **A sort switch changes which pages follow which.** Settings are stored per file, but "following"
  means the current order. If name order and date order differ, switching re-assigns pages without
  their own settings to a different predecessor. Row 0001.6f asks before switching; it does not prevent
  it. Photos renamed to a sortable scheme before starting avoid the question.

- **`pagecrop.json` is the only copy of the user's work.** It is saved by write-then-rename, an
  unreadable file is kept as `.bak` and never overwritten silently, and settings of files that are
  missing are kept as orphans. Deleting or renaming photos between sessions is safe; renaming them
  detaches their settings, and nothing in this roadmap re-attaches them.

- **Rotating and then shrinking resamples twice.** The output is slightly softer than a single pass
  would give. At a long edge of 1600 from a 12-megapixel photo this should not be visible. The signal
  to revisit it: small print looks soft at 100 % in the exported file compared with the original shrunk
  in an image editor.

- **Export memory grows with photo size and thread count.** Roughly 150 MB per worker for a
  12-megapixel photo, four times that for 50 megapixels, with up to eight workers. The cap of eight is
  the mitigation. The signal to revisit: the machine swaps during export, or row 0001.17 measures fewer
  than 5 photos per second.

- **White fills whatever the frame covers outside the photo.** That suits book pages. A frame pushed
  past the edge of a photo of something dark will show white wedges, and nothing warns about it.
