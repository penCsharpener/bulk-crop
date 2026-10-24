## Phase 0002 — Two-sided books, deleting photos, clean straightening (P1)

Phase 0001 is finished. Using it on real books showed five gaps:

1. **Bad shots stay in the list.** A blurred photo or a photo taken twice has to be deleted in a file
   manager, and the folder reopened.
2. **Left and right pages need different frames.** A book photographed page by page gives left page,
   right page, left, right… The page sits further left in one half of the photos and further right in
   the other, and the two halves tilt differently. One frame that flows forward fits only one half.
3. **The frame handles are hard to grab.** They react within 8 screen points. On a large screen the
   user has to hunt for the edge.
4. **Brightening greys the text.** Brightness adds the same amount to every level, so black ink turns
   grey as fast as the paper turns white.
5. **A turned picture sticks out of its box, and the frame can reach the white corners.** The editor
   draws the turned photo with its corners poking over the dark surround. The frame may cover the
   white wedges the turn leaves, and they end up in the export.

The end state:

- A bin button moves the selected photo to the desktop trash, after one confirmation.
- A toggle **⇄ Alternating sides** gives every second page its own frame and fine angle. Turning it off
  on a later page affects only that page and the pages after it.
- Each handle of the frame reacts within a zone of a set percentage of the picture (default 5 %),
  without being drawn bigger. The percentage is a setting that the app remembers.
- Brightness is a gamma curve. Black stays black and white stays white.
- The turned picture is clipped to its upright rectangle. The frame always stays inside the turned
  picture, so no export has white corners.

The user decided the following, and they are not open for discussion in this roadmap:

- Delete means **move to the desktop trash, after a confirmation**. Nothing is deleted for good.
- In alternating mode, **frame and fine angle** differ per side. Quarter turns and tone stay shared.
- The handle-reach setting is **app-wide**, not per folder.
- When the angle changes, the picture is **clipped and the frame auto-shrinks** so it stays inside.

Read `CLAUDE.md`, `docs/Roadmaps/readme.md` and the section "The model and decisions" of
`0001 Book page cropper.md` first. This file only describes what changes.

## Before the first row

- All of phase 0001 is marked ✅. Run the three gates once on a clean checkout. They must pass before
  anything here starts.
- Make the sample book if it is missing: `cargo run -p pagecrop-cli -- gen-fixtures samples/book`.
- New dependencies, added in the row that first needs them, never earlier:
  - `cargo add trash -p pagecrop-core` (row 0002.5)
  - `cargo add eframe -p pagecrop --features persistence` (row 0002.3f; this only turns on a feature
    of the crate the app already uses)

## Skills this work needs

| Skill | Rows |
|---|---|
| `roadmap-row` | every row |
| `image-io` | 0002.1, 0002.4 (testkit part) |
| `image-geometry` | 0002.2, 0002.2f, 0002.3, 0002.4 |
| `egui-app` | every `f` row |

Rows 0002.1 and 0002.2 change what these skills say. Update the skill text in the same commit, so the
next agent does not read the old formula.

## The model and decisions

### What changes in the types

```rust
// model.rs — one new field. Every other field stays as it is.
pub struct Adjust {
    pub quarter_turns: u8,
    pub angle_deg: f32,
    pub crop: CropRect,
    pub brightness: i32,
    pub contrast: i32,
    pub grayscale: bool,
    /// From this page on, odd and even pages keep their own frame and fine angle.  (0002.4)
    pub alternate: bool,          // default false; #[serde(default)] on the struct already covers old files
}

// error.rs — one new variant
#[error("cannot move {} to the trash: {message}", path.display())]
Trash { path: PathBuf, message: String },
```

Adding a field to `Adjust` breaks every struct literal of `Adjust` that does not end in
`..Adjust::default()`. Known places: `normalized_clamps_all_fields` in `model.rs` and the `render`
command in `crates/cli/src/main.rs`. Add `alternate: false` there. Do not add `..Default::default()`
where the code lists every field on purpose.

### New and changed functions

| Module | Row | Item |
|---|---|---|
| `tone.rs` | 0002.1 | `lut` keeps its signature; its formula changes to a gamma curve |
| `geometry.rs` | 0002.2 | `point_inside`, `frame_inside`, `fit_inside` |
| `crop_edit.rs` | 0002.2 | `drag_inside` |
| `crop_edit.rs` | 0002.3 | `handle_reach` |
| `geometry.rs` | 0002.4 | `turn_crop` |
| `model.rs` | 0002.4 | `Project::effective` is rewritten (same signature); new `Project::side` |
| `model.rs` | 0002.5 | `Project::remove_page` |
| `delete.rs` (new) | 0002.5 | `move_to_trash` |
| testkit `lib.rs` | 0002.4 | `page_image_at`, `write_two_sided_book` |

No other module, trait or generic type is added.

### How alternating sides work

This is the hardest part of the phase. Read it twice.

Nothing new is stored except `alternate: bool` on a page's own settings (its *key*). The rest follows
from the keys and their order, as the flow-forward rule of phase 0001 does.

- A **run** is a stretch of keys in a row that all have `alternate == true`. The run's **anchor** is its
  first key. A key with `alternate == false`, or no key at all, ends the run.
- A page inside a run is on **side 0** when `(page − anchor)` is even and on **side 1** when it is odd.
  On screen the sides are called **A** and **B**. The app does not know which side is the left page.
- **Shared fields** (`quarter_turns`, `brightness`, `contrast`, `grayscale`, `alternate`) come from
  the nearest key at or before the page, exactly as in phase 0001.
- **Side fields** (`angle_deg`, `crop`) come from the nearest key at or before the page **on the same
  side** within the run. When side B has no key yet, it uses the anchor's, so side B starts as a copy
  of side A.

```
page:        1   2   3   4   5   6   7   8   9   10  11  12
key:         -   -   A*  B   -   -   C   -   -   D   -   -        * = alternate turned on here
                         (B: alternate, own frame)  (C: side A edited)  (D: alternate off)
side:        -   -   A   B   A   B   A   B   A   B   -   -
frame from:  d   d   3   4   3   4   7   4   7   10  10  10
tone from:   d   d   3   4   4   4   7   7   7   10  10  10
```

What this gives the user, without any special code in the editor:

- Editing a page does `set_key(i, changed copy of effective(i))`, as before. `effective(i)` already holds
  the frame and angle of page `i`'s side, so the new key carries the right side's values.
- Editing a side-A page never moves a side-B frame, and the other way round.
- Turning alternating **off** on page 10 sets a key with `alternate: false`. The run ends at page 9, so
  pages 3 to 9 keep alternating and pages 10 onward use page 10's frame. That is what the user asked
  for.
- Turning it **on** on page 3 sets a key with `alternate: true`. Page 4 shows the same frame until the
  user changes it.
- Undo and redo need nothing new, because `Edit` stores whole keys.
- Export needs nothing new, because `plan` calls `effective`.

Parity counts positions in the current list. Deleting a photo inside a run therefore moves every later
page to the other side. That is right for the most common reason to delete in such a book, a photo
taken twice: before the delete, every later page has the wrong frame; after it, they are right again.
See Risks for the other case.

**Quarter turns inside a run.** A side-B frame can come from a key with different `quarter_turns` than
the nearest key (for example, after the user pressed ⟳ 90° on a side-A page). Frames are fractions of
the *turned* picture, so the B frame must be turned by the difference first. `turn_crop` does this
(snippet S4).

### How the frame stays inside a turned picture

The picture after its quarter turns is the **base**, `base.w × base.h` pixels. The fine angle turns it
about its centre. On screen it is now clipped to the upright base rectangle. The frame, in fractions of
the base, must lie inside **both**:

1. the upright base rectangle: every corner has `|x| ≤ 0.5` and `|y| ≤ 0.5`, and
2. the turned picture: every corner, turned back by the fine angle, lies within the base rectangle.

Both regions are convex, so their intersection is convex. This is why a binary search along a drag is
correct (snippet S3).

The "turn back" is the same formula `frame_to_src` uses, with only the fine angle:
`qx = x·cos + y·sin`, `qy = −x·sin + y·cos`, applied to pixel offsets from the centre. Use exactly this
and do not invent a second version. If the sign is wrong, only off-centre frames fail, and the test
`fitted_render_has_no_white_corners` is there to catch that.

`fit_inside` shrinks the frame about its own centre and keeps its shape. It never grows the frame.
During one slider drag the app fits from the frame as it was when the drag began, so dragging the angle
to 5° and back to 0° gives the original frame back. Separate nudges with `[` `]` each fit from the
current frame and do not grow it back. That is accepted.

### Why not the other way

- **Store a second `CropRect` and angle inside each key** ("this side" and "other side"). Every key would
  then also freeze the other side's frame, so editing a side-A page after a side-B key could no longer
  flow to later side-A pages. Looking up the nearest same-side key gives independent sides for free.
- **Parity by absolute page index.** Turning alternating on at page 4 would then make page 4 side B.
  Counting from the anchor makes the page the user toggled on always side A.
- **Delete for good.** One wrong click would lose an original photo. The trash keeps it restorable,
  and `CLAUDE.md` says originals are precious.
- **Keep the undo history across a delete.** It names pages by index, and a delete moves pages. Shifting
  indices is possible but interacts with the key that moves to the next page. Clearing is simple and
  the confirmation says so.
- **Bigger drawn handles.** The user asked for a bigger grab zone that looks the same.
- **Shrink the frame only at export.** The export would no longer match what the editor shows.

## Code the rows rely on

These are the places where a small mistake gives a result that looks plausible but is wrong. Copy them,
then make them pass `cargo fmt` and `clippy`. Doc comments are part of the code.

### S1 — gamma brightness (`tone.rs`, row 0002.1)

```rust
/// The output level for every input level, from brightness and contrast (each -100 to 100).
/// Brightness is a gamma curve: 0 and 255 stay where they are, and the middle moves most.
/// Contrast then stretches the levels about mid-grey.
pub fn lut(brightness: i32, contrast: i32) -> [u8; 256] {
    // +100 gives gamma 1/3, -100 gives gamma 3, 0 gives 1.
    let gamma = 3.0f32.powf(-(brightness.clamp(-100, 100) as f32) / 100.0);
    let k = 1.0 + contrast as f32 / 100.0;
    let mut table = [0u8; 256];
    for (v, out) in table.iter_mut().enumerate() {
        let lifted = 255.0 * (v as f32 / 255.0).powf(gamma);
        let x = (lifted - 127.5) * k + 127.5;
        *out = x.round().clamp(0.0, 255.0) as u8;
    }
    table
}
```

Worked values for the tests: `lut(100, 0)[128] == 203`, `lut(-100, 0)[128] == 32`,
`lut(50, 0)[30] == 74` (the old formula gave 94).

### S2 — inside tests and fitting (`geometry.rs`, row 0002.2)

```rust
/// Slack for the inside tests, in fractions of the base picture, so a frame fitted exactly to an
/// edge still counts as inside after rounding.
const INSIDE_EPS: f32 = 1e-4;

/// The four corners of a frame as signs: left/right, top/bottom.
const CORNERS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

/// Turns an offset from the picture centre back by the fine angle, the same way `frame_to_src` does.
fn turn_back(angle_deg: f32) -> impl Fn(f32, f32) -> (f32, f32) {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    move |x, y| (x * cos + y * sin, -x * sin + y * cos)
}

/// Whether a point, in fractions of the base picture from its centre, lies inside the upright base
/// rectangle and inside the picture turned by `angle_deg`.
pub fn point_inside(p: (f32, f32), base: Size, angle_deg: f32) -> bool {
    let (x, y) = p;
    let upright = x.abs() <= 0.5 + INSIDE_EPS && y.abs() <= 0.5 + INSIDE_EPS;
    let (qx, qy) = turn_back(angle_deg)(x * base.w, y * base.h);
    let turned = qx.abs() <= base.w * (0.5 + INSIDE_EPS) && qy.abs() <= base.h * (0.5 + INSIDE_EPS);
    upright && turned
}

/// Whether all four corners of the frame are inside (see `point_inside`).
pub fn frame_inside(crop: CropRect, base: Size, angle_deg: f32) -> bool {
    CORNERS.iter().all(|&(sx, sy)| {
        let corner = (crop.cx + sx * crop.w / 2.0, crop.cy + sy * crop.h / 2.0);
        point_inside(corner, base, angle_deg)
    })
}

/// The frame shrunk about its centre, keeping its shape, until it lies inside the upright base
/// rectangle and inside the picture turned by `angle_deg`. A frame already inside comes back
/// unchanged; the frame never grows. A frame whose centre is outside is first moved to the middle.
pub fn fit_inside(crop: CropRect, base: Size, angle_deg: f32) -> CropRect {
    let crop = if point_inside((crop.cx, crop.cy), base, angle_deg) {
        crop
    } else {
        CropRect { cx: 0.0, cy: 0.0, ..crop }
    };
    let mut s: f32 = 1.0;
    // Upright rectangle: |c ± s·half| <= 0.5 on each axis.
    for (c, half) in [(crop.cx, crop.w / 2.0), (crop.cy, crop.h / 2.0)] {
        if half > 0.0 {
            s = s.min((0.5 - c.abs()) / half);
        }
    }
    // Turned picture, in pixels: u + s·v must stay within -l..=l for every corner and axis,
    // where u is the turned centre and v the turned corner offset.
    let turn = turn_back(angle_deg);
    let (ux, uy) = turn(crop.cx * base.w, crop.cy * base.h);
    let (lx, ly) = (base.w / 2.0, base.h / 2.0);
    for (sx, sy) in CORNERS {
        let (vx, vy) = turn(sx * crop.w * base.w / 2.0, sy * crop.h * base.h / 2.0);
        for (u, v, l) in [(ux, vx, lx), (uy, vy, ly)] {
            if v > 1e-6 {
                s = s.min((l - u) / v);
            } else if v < -1e-6 {
                s = s.min((-l - u) / v);
            }
        }
    }
    let s = s.clamp(0.0, 1.0);
    CropRect { w: crop.w * s, h: crop.h * s, ..crop }
}
```

`turn_back` returns `impl Fn`. If clippy objects, write it as a plain
`fn turn_back(angle_deg: f32, x: f32, y: f32) -> (f32, f32)` instead. Do not change the formula.

### S3 — dragging that stays inside (`crop_edit.rs`, row 0002.2)

```rust
/// Like `drag`, but the frame also stays inside the turned picture (see `geometry::frame_inside`),
/// so it never covers the white corners a fine angle leaves. A start frame that is not inside is
/// fitted first. When the whole movement would leave the picture, the frame goes as far as it can,
/// then slides along the edge on each axis alone.
pub fn drag_inside(
    start: CropRect,
    handle: Handle,
    delta: (f32, f32),
    base: Size,
    angle_deg: f32,
    min: (f32, f32),
) -> CropRect {
    let start = fit_inside(start, base, angle_deg);
    let at = |d: (f32, f32)| drag(start, handle, d, (0.5, 0.5), min);
    let ok = |d: (f32, f32)| frame_inside(at(d), base, angle_deg);
    if ok(delta) {
        return at(delta);
    }
    let t = furthest(|t| ok((delta.0 * t, delta.1 * t)));
    let mut reached = (delta.0 * t, delta.1 * t);
    let tx = furthest(|s| ok((reached.0 + (delta.0 - reached.0) * s, reached.1)));
    reached.0 += (delta.0 - reached.0) * tx;
    let ty = furthest(|s| ok((reached.0, reached.1 + (delta.1 - reached.1) * s)));
    reached.1 += (delta.1 - reached.1) * ty;
    at(reached)
}

/// The largest `t` in `0..=1` for which `ok(t)` holds. `ok(0)` must hold, and `ok` may switch from
/// true to false only once; both are true here because the allowed region is convex.
fn furthest(ok: impl Fn(f32) -> bool) -> f32 {
    if ok(1.0) {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if ok(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}
```

`drag` already clamps to `(0.5, 0.5)`, the upright rectangle, so `drag_inside` passes that and never
`half_extents`. `half_extents` stays in the crate. Do not delete it in this phase.

### S4 — alternating lookup (`model.rs` and `geometry.rs`, row 0002.4)

```rust
// geometry.rs
/// A frame made for a picture with `from` quarter turns, turned to fit the same picture with `to`
/// quarter turns, so it frames the same part of the page.
pub fn turn_crop(crop: CropRect, from: u8, to: u8) -> CropRect {
    let steps = (to % 4 + 4 - from % 4) % 4;
    let mut a = Adjust { quarter_turns: from % 4, crop, ..Adjust::default() };
    for _ in 0..steps {
        a = quarter_turn(a, true);
    }
    a.crop
}
```

```rust
// model.rs — replaces the body of `effective`; the signature stays.
/// The settings that apply to page `i`. The default when none applies or `i` is out of range.
/// Inside a run of alternating keys the frame and the fine angle come from the nearest key on
/// the same side; everything else comes from the nearest key.
pub fn effective(&self, i: usize) -> Adjust {
    let Some(k) = self.key_index(i) else {
        return Adjust::default();
    };
    let Some(own) = self.key_at(k) else {
        return Adjust::default();
    };
    if !own.alternate {
        return own;
    }
    let anchor = self.run_start(k);
    let side = self.key_at(self.side_source(anchor, i)).unwrap_or(own);
    Adjust {
        angle_deg: side.angle_deg,
        crop: turn_crop(side.crop, side.quarter_turns, own.quarter_turns),
        ..own
    }
}

/// Which side page `i` is on inside a run of alternating keys: 0 for the side of the run's first
/// page, 1 for the other. None when page `i` does not alternate or is out of range.
pub fn side(&self, i: usize) -> Option<u8> {
    let k = self.key_index(i)?;
    if !self.key_at(k)?.alternate {
        return None;
    }
    let anchor = self.run_start(k);
    Some((i.saturating_sub(anchor) % 2) as u8)
}

fn key_at(&self, i: usize) -> Option<Adjust> {
    self.entries.get(i).and_then(|e| e.key)
}

/// The first key of the run of alternating keys that ends at key `k`.
fn run_start(&self, k: usize) -> usize {
    let mut start = k;
    while let Some(prev) = self.keyed_page_from(start, false) {
        if !self.key_at(prev).is_some_and(|a| a.alternate) {
            break;
        }
        start = prev;
    }
    start
}

/// The page whose frame and fine angle page `i` uses inside the run that starts at `anchor`: the
/// nearest page at or before `i` on the same side that has its own settings, else the anchor.
fn side_source(&self, anchor: usize, i: usize) -> usize {
    let parity = i.saturating_sub(anchor) % 2;
    (anchor..=i)
        .rev()
        .filter(|j| (j - anchor) % 2 == parity)
        .find(|&j| self.has_key(j))
        .unwrap_or(anchor)
}
```

`(j - anchor)` cannot underflow because `j` runs from `anchor` upward. If clippy still complains, use
`j.saturating_sub(anchor)`.

### S5 — removing a page (`model.rs`, row 0002.5)

```rust
/// Takes page `i` out of the list and returns it. When the page had its own settings and the page
/// after it has none, the settings move to that page, so the pages that followed it look the same.
/// None when `i` is out of range.
pub fn remove_page(&mut self, i: usize) -> Option<ImageEntry> {
    if i >= self.entries.len() {
        return None;
    }
    let removed = self.entries.remove(i);
    if let (Some(key), Some(next)) = (removed.key, self.entries.get_mut(i))
        && next.key.is_none()
    {
        next.key = Some(key);
    }
    Some(removed)
}
```

`Vec::remove` panics out of range. The `if` above it is the guard. Keep it first.

### S6 — the app-wide setting (`app.rs`, row 0002.3f)

eframe 0.36.2 with the `persistence` feature has these. They were checked in the installed source,
`eframe-0.36.2/src/epi.rs`:

```rust
// CreationContext field:   pub storage: Option<&'s dyn eframe::Storage>
// free functions:          eframe::get_value::<T>(storage: &dyn Storage, key: &str) -> Option<T>
//                          eframe::set_value::<T>(storage: &mut dyn Storage, key: &str, value: &T)
// App trait method:        fn save(&mut self, storage: &mut dyn eframe::Storage)

/// The key under which the handle reach is stored in eframe's settings.
const REACH_KEY: &str = "handle_reach_percent";

// in App::new, after the struct is built:
if let Some(storage) = storage {
    if let Some(p) = eframe::get_value::<f32>(storage, REACH_KEY) {
        app.handle_reach_percent = p.clamp(REACH_RANGE.0, REACH_RANGE.1);
    }
}

// in `impl eframe::App for App`:
fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, REACH_KEY, &self.handle_reach_percent);
}
```

`App::new` gains a parameter `storage: Option<&dyn eframe::Storage>`. Its only caller is `main.rs`,
which passes `cc.storage`. This changes a signature from row 0001.3f. The `CLAUDE.md` rule allows it
because the only caller changes in the same commit.

## Tasks

| # | Task | Depends on | Done when |
|---|------|-----------|-----------|
| ✅ 0002.1 | **Gamma brightness** (core; the screen needs no change, the sliders already call `lut` through `render`). Skill: image-io.<br>`tone.rs`: replace the body of `lut` with snippet S1. Same signature.<br>Tests: delete `brightness_100_maps_128_to_255`, which pins the old formula. Add `brightness_keeps_black_and_white` (for brightness in −100, −50, 50, 100: `lut(b, 0)[0] == 0` and `lut(b, 0)[255] == 255`), `brightness_100_lifts_mid_grey_to_203`, `brightness_minus_100_darkens_mid_grey_to_32`, `brightness_lifts_ink_less_than_before` (`lut(50, 0)[30] == 74`), `brightness_table_is_monotonic` (brightness 80 and −80, contrast 0). Keep `zero_zero_is_identity_table`, `contrast_minus_100_flattens_to_mid_grey`, `table_is_monotonic_for_positive_contrast`; they must still pass unchanged.<br>Update the "Tone" section of `.claude/skills/image-io/SKILL.md` to the new formula. Add one line to the roadmap Risks if a test outside `tone.rs` breaks. | — | `cargo test -p pagecrop-core tone` is green. In the app, on `samples/book` page 1, brightness +60: the paper turns whiter and the text lines stay clearly darker than with the old build at the same value. Compare with `git stash`, or with a screenshot taken before the change. |
| ✅ 0002.2 | **Frame stays inside the turned picture** (core). Skill: image-geometry.<br>`geometry.rs`: `point_inside`, `frame_inside`, `fit_inside` from snippet S2. `crop_edit.rs`: `drag_inside` and the private `furthest` from snippet S3.<br>Tests in `geometry.rs`: `full_frame_at_zero_angle_is_unchanged`; `fit_keeps_shape_and_centre` (frame cx 0.1, cy −0.05, w 0.6, h 0.8 at 6° on 1600×1200: same cx, cy and same w/h ratio within 1e-4, smaller w); `fitted_frame_is_inside` (for angles −10, −3, 0.5, 7, 10 and three frames, full, off-centre and tall: `frame_inside(fit_inside(..))`); `full_frame_at_5_degrees_is_not_inside`; `fit_never_grows` (a small frame well inside comes back equal); `centre_outside_is_moved_to_middle`; `fitted_render_has_no_white_corners`: source `RgbImage::from_pixel(800, 600, Rgb([100, 100, 100]))`, angle 4°, frame cx 0.1, cy −0.1, w 0.9, h 0.9, fitted, then `render::render_rgb(.., Resize::None)`. Every pixel at least 1 px away from the border has red < 200. This test pins the turn direction against the real renderer.<br>Tests in `crop_edit.rs`: `drag_inside_at_zero_angle_equals_drag` (E handle, delta 0.3, compared with `drag` and half (0.5, 0.5)); `drag_inside_stops_at_turned_edge` (base 1600×1200, 6°, frame cx 0, cy 0, w 0.5, h 0.5, E handle delta (1.0, 0.0): result `frame_inside`, right edge ≈ 0.4831 within 1e-3, so it stopped at the slanted edge before the upright bound 0.5); `drag_inside_slides_along_edge` (same base and angle, frame cx 0.3, cy 0, w 0.2, h 0.2, Move by (0.5, 0.1): result inside, cx ≈ 0.3935 and cy ≈ 0.0187 within 1e-3; these values come from running snippets S2 and S3 in a float simulation).<br>Update `.claude/skills/image-geometry/SKILL.md` with a short section "Frame stays inside", copying the two bullet points from "How the frame stays inside a turned picture" above. | 0002.1 | `cargo test -p pagecrop-core geometry crop_edit` is green, including all ten new tests. |
| ✅ 0002.2f | **Clipped picture and fitted frame** (screen). Skills: egui-app, image-geometry.<br>`editor.rs`:<br>• `Layout` gains `fn base_rect(&self) -> egui::Rect` = `Rect::from_center_size(self.centre, self.base_points())`.<br>• `paint_picture`: first `painter.rect_filled(base_rect, 0.0, Color32::WHITE)` (the wedges are white, as in the export), then paint the mesh with `painter.with_clip_rect(layout.base_rect())`. That painter intersects with the area's clip, so zoom and pan still work.<br>• `handle_pointer`: replace `drag_frame(.., half_extents(..), MIN_FRAME)` with `drag_inside(drag.start, drag.handle, delta, layout.base, adjust.angle_deg, MIN_FRAME)`. Remove the `half_extents` import if it becomes unused.<br>`app.rs`:<br>• New field `pub angle_start: Option<CropRect>`: the frame when the current angle drag began.<br>• New `pub fn selected_base(&self) -> Option<Size>`: `full_size` of the selected `Content::Picture` turned by `base_size(.., effective().quarter_turns)`. None while the preview is loading.<br>• New `pub fn angled(&self, angle_deg: f32) -> Adjust`: the effective settings with this angle, and the frame `fit_inside(self.angle_start.unwrap_or(current.crop), base, angle_deg)`. When `selected_base()` is None, keep the frame.<br>• `nudge_angle` builds its `Adjust` with `angled(..)`.<br>`panels/adjust.rs`, `rotation`: on `slider.drag_started()` set `app.angle_start = Some(current.crop)`. Pass `app.angled(angle)` to `track`. On `slider.drag_stopped()` set it back to None. The "0°" button uses `app.angled(0.0)`. | 0002.2 | On `samples/book` page 6, set the fine angle to −4°. The picture's corners no longer stick out over the dark surround; it is cut at the upright rectangle, with white wedges in the corners. The frame shrinks so it never covers a wedge. Drag the angle slider from 0 to 8° and back to 0 in one drag: the frame returns to its old size. Drag the frame's right edge outward: it stops at the slanted picture edge, not at the corner. "Show result" (P) shows no white corners. Undo restores the previous frame and angle in one step. |
| ✅ 0002.3 | **Handle reach** (core). Skill: image-geometry.<br>`crop_edit.rs`: `pub fn handle_reach(base: Size, crop: CropRect, percent: f32, floor: (f32, f32)) -> (f32, f32)`. It returns the reach on each axis in frame fractions: `percent` per cent of the base picture's **shorter** side (so the zone is square on screen), capped at a third of the frame's width (or height) so the middle of the frame still moves it, and never below `floor`. A base side ≤ 0 returns `floor`. Order: `(reach / base.w).min(crop.w / 3.0).max(floor.0)`, and the same for y.<br>Tests: `reach_is_percent_of_shorter_side` (1600×1200, full frame, 5 % → (0.0375, 0.05)); `reach_is_capped_by_small_frame` (1600×1200, 5 %, frame 0.09 × 0.09 → (0.03, 0.03)); `reach_never_below_floor`; `centre_of_frame_still_moves_with_wide_reach` (`hit_test` at the frame centre with that reach gives `Move`). | 0002.2 | `cargo test -p pagecrop-core crop_edit` is green, with the four new tests. |
| ✅ 0002.3f | **Handle reach setting** (screen). Skill: egui-app.<br>`cargo add eframe -p pagecrop --features persistence`. Commit `Cargo.lock`.<br>`app.rs`: field `pub handle_reach_percent: f32`, default 5.0, with `const REACH_RANGE: (f32, f32) = (1.0, 15.0)`. Load and save it as in snippet S6, and pass `cc.storage` in `main.rs`.<br>`editor.rs`: replace `fn reach(layout)` with one that calls `handle_reach(layout.base, crop, app.handle_reach_percent, floor)`, where `floor` = 8 screen points in fractions, i.e. the old `(HANDLE_REACH / base.x, HANDLE_REACH / base.y)`. `reach` now also needs the frame and the percent, so pass them in. `view_input` and `handle_pointer` both call it. The drawn handles and outline do **not** change.<br>`panels/adjust.rs`, section "View": a slider "Handle reach" from 1 to 15 with suffix " %", step 0.5, with the hover text "How close the pointer must be to an edge or corner of the frame, as a share of the picture". It changes `app.handle_reach_percent` only. It is not a page setting, so it does not go through `track` or the history. | 0002.3 | Grabbing an edge works from visibly further away than before, about 5 % of the picture's shorter side, while the frame looks exactly as before. A small frame can still be moved by dragging its middle. Set the slider to 12 %, close the app and start it again: the slider still shows 12 %. Opening another folder keeps 12 %. |
| 0002.4 | **Alternating sides** (core + testkit + command line). Skills: image-geometry, image-io. Read "How alternating sides work" first.<br>`model.rs`: add `alternate: bool` to `Adjust` (default false; `normalized` passes it through). Add the private helpers `key_at`, `run_start`, `side_source` and the public `side` from snippet S4, and rewrite the body of `effective` with it. Fix the struct literals listed under "What changes in the types".<br>`geometry.rs`: `turn_crop` from snippet S4.<br>Tests in `model.rs` (pages numbered from 1, as in the existing tests; use a helper `frame(cx)` that builds a key with a distinct `crop.cx`): `alternating_pages_use_two_frames` (keys on 3 = alternate, frame A, angle 1; on 4 = alternate, frame B, angle −1: pages 5, 7, 9, 11 have A and 1; pages 6, 8, 10, 12 have B and −1); `side_b_starts_as_copy_of_side_a` (only key 3: page 4 has A); `tone_on_one_side_flows_to_both` (keys 3, 4 as above, then a key on 6 = `effective(6)` with brightness 30: page 7 has brightness 30 and frame A); `editing_side_a_leaves_side_b` (keys 3, 4, then a key on 7 = `effective(7)` with frame C: pages 7, 9, 11 have C; pages 8, 10, 12 have B); `turning_off_keeps_earlier_pages_alternating` (keys 3, 4, then a key on 10 = `effective(10)` with `alternate: false`: pages 3–9 alternate A/B as before; pages 10–12 all have B; `side` of page 10 is None); `side_frames_follow_quarter_turn` (key 3 alternate frame cx 0.2, cy 0.0, w 0.5, h 0.9, turns 0; key 5 = `quarter_turn(effective(5), true)`: page 6 has `turn_crop` of that frame from 0 to 1); `side_of_page` (with keys 3, 4: `side` of pages 2, 3, 4, 5 is None, 0, 1, 0); `pages_without_alternate_behave_as_before` (the existing flow-forward tests stay green unchanged; this test just repeats `key_on_6_between_two_keys_stops_before_8` with `alternate: false` written out).<br>Test in `persist.rs`: `file_without_alternate_loads_as_false` (a key written without the field loads with `alternate == false`).<br>Test in `geometry.rs`: `turn_crop_by_zero_is_identity`, `turn_crop_round_trip` (from 1 to 3, then from 3 to 1, gives the original).<br>Testkit: `page_image_at(w, h, index, angle_deg, shift)` is `page_image` with the page centre moved right by `shift × w`. `page_image` becomes a call to it with shift 0, keeping its signature. `write_two_sided_book(dir, count, w, h) -> io::Result<Vec<PathBuf>>`: odd photos (1, 3, …) shift +0.08 and turn +1.5°, even photos shift −0.08 and turn −1.0°; names and JPEG writing as in `write_book`. Test `two_sided_pages_sit_left_and_right`: on in-memory images, the pixel at (0.15·w, 0.5·h) is the table colour on photo 1 and not on photo 2.<br>CLI: `gen-fixtures` gains `--two-sided`, which calls `write_two_sided_book`. | 0002.3f | `cargo test --workspace` is green, with every named test. `cargo run -p pagecrop-cli -- gen-fixtures samples/two-sided --two-sided --count 12` writes 12 photos whose pages alternate left and right. The app still runs and behaves exactly as before on `samples/book`. |
| 0002.4f | **Alternating toggle** (screen). Skills: egui-app.<br>`panels/adjust.rs`: a new section "Sides" between "Rotation" and "View" with `ui.toggle_value(&mut on, "⇄ Alternating sides")`, where `on = app.effective().alternate`. On change: `app.edit_once(Adjust { alternate: on, ..current })`. Hover text: "Every second page keeps its own frame and angle. Turning it off here affects this page and the pages after it." Below it, when `project.side(selected)` is Some, a label "This page: side A" or "side B".<br>`panels/pages.rs`: when `side(index)` is Some, paint "A" or "B" in small text at the row's top-right corner, using `ACCENT` for B and the normal text colour for A.<br>`app.rs`, `page_label`: append " · side A" or " · side B" when the page alternates.<br>`editor.rs`: when the page alternates, paint "Side A" or "Side B" in the top-left corner of the canvas area, on a half-transparent dark rectangle so it reads on white.<br>`help.rs` stays unchanged: the toggle has no shortcut. | 0002.4 | `cargo run -p pagecrop -- samples/two-sided`: on page 1, set a frame around the page and turn on "⇄ Alternating sides". Page 2 shows the same frame and is marked B. Move page 2's frame onto its page and straighten it. Now pages 3, 5, 7 show page 1's frame and angle, and pages 4, 6, 8 show page 2's. Change page 5's frame: pages 7, 9, 11 follow and the B pages do not move. On page 9, turn the toggle off: pages 9–12 use page 9's frame, and pages 1–8 still alternate. Export: odd and even outputs are framed on their own pages. Restarting the app restores all of it. |
| 0002.5 | **Remove a page and move a photo to the trash** (core).<br>`cargo add trash -p pagecrop-core`. Commit `Cargo.lock`.<br>`error.rs`: variant `Trash { path, message }` as in "What changes in the types".<br>New module `delete.rs` (add `pub mod delete;` to `lib.rs`): `pub fn move_to_trash(path: &Path) -> Result<(), CoreError>`. First, if `!path.is_file()`, return `CoreError::Io` with `std::io::ErrorKind::NotFound` and the path, so a missing file never reaches the trash crate. Then call the trash crate (`trash::delete(path)` in 5.x; check the docs of the installed version) and map its error to `CoreError::Trash`.<br>`model.rs`: `Project::remove_page` from snippet S5.<br>Tests in `delete.rs`: `missing_file_is_error_with_path` (the error message contains the path). Do not write a test that really moves a file to the trash: the result depends on the machine's trash folders. Check that by hand below.<br>Tests in `model.rs`: `removing_keyed_page_moves_key_to_next` (12 pages, key on 4, remove page 4: new page 4, formerly 5, has that key; pages 4–11 see it); `removing_keyed_page_before_keyed_page_drops_key` (keys on 4 and 5, remove 4: new page 4 keeps page 5's key); `removing_plain_page_keeps_keys`; `removing_doubled_shot_restores_sides` (7 pages, key 1 = alternate frame A, key 2 = alternate frame B; page 4 is a double of page 3, so before removal page 5 has frame A. After `remove_page(3)` (page 4), the new page 4, formerly 5, has frame B and new page 5 has A); `remove_out_of_range_is_none`.<br>`CLAUDE.md`, section "Data that must not be lost": change the first bullet to: "Original photos are never written to. The only exception is the bin button: after the user confirms, it moves one photo to the desktop trash with `delete::move_to_trash`. Nothing else may delete, move or change a photo." Change the same rule in `README.md` if it states it. | 0002.4f | `cargo test --workspace` is green, with the six new tests. `CLAUDE.md` carries the new rule. |
| 0002.5f | **Bin button** (screen). Skill: egui-app.<br>`panels/top.rs`: after the redo button and its separator, a button "🗑" (the egui icon font has it; if it shows as a box, write "Delete" instead) with hover text "Move this photo to the trash". It is disabled when there are no pages or an edit is in progress (`app.editing()`).<br>Shortcut **Shift+Delete** does the same. Plain Delete keeps resetting the page. In `edit_keys`, check Shift+Delete **before** the existing Delete branch, and make the existing branch require `!shift`. Add `("Shift+Delete", "Move the photo to the trash")` to `help.rs`.<br>`app.rs`:<br>• Fields `pending_delete: bool`, `ask_before_delete: bool` (true at start; not saved).<br>• `request_delete()` sets `pending_delete`, or deletes at once when `ask_before_delete` is false.<br>• `confirm_delete(ctx)`, modelled on `confirm_sort`: a centred window "Move to trash" that names the file and says "This also clears undo." It has a checkbox "Don't ask again until the app closes" and buttons "Move to trash" and "Cancel". Escape cancels. Call it in `ui` next to `confirm_sort`.<br>• `delete_selected()`, in this order: (1) take the selected entry's path; (2) `move_to_trash(&path)`; on `Err`, put the message into `self.error` and stop, so nothing else changes; (3) `project.remove_page(selected)`; (4) remove the same index from `self.thumbs`, guarded by `if selected < self.thumbs.len()`; (5) `history.clear()`, `edit_start = None`, `drag = None`; (6) `preview_generation += 1`, `previews.clear()`, `last_requested = None`, `result = None`, `tone = None`; (7) `self.select(self.selected)`, which clamps when the last page was deleted, and `reveal_selected = true`; (8) `save_settings()`.<br>The deleted file's settings are not kept as an orphan: the user removed the photo on purpose. | 0002.5 | Copy `samples/book` to `samples/del` and open it. Select page 5 (with a key of its own): click 🗑 and confirm. The photo is gone from the list and from the folder, and appears in the desktop trash (`ls ~/.local/share/Trash/files`). The page after it is selected and shows the same frame as before. Shift+Delete on another page asks again; tick "Don't ask again"; the next Shift+Delete deletes at once. Restart: the deleted photos stay gone and every other page's settings are intact. On `samples/two-sided` with alternating on, deleting a double shot puts the later pages back on the right side. Cancel deletes nothing. |
| 0002.6 | **Final check on a two-sided book** (no code unless a bug turns up; a bug fix gets its own commit `0002.6: fix …`).<br>Make `samples/final` with `gen-fixtures --two-sided --count 40`. Copy photo 17 to `page_017b.jpg`, a double shot. Then, in the app: set alternating frames on pages 1 and 2 and straighten both; brighten +40; delete the double; turn alternating off from page 30 and set one frame there; set handle reach to 8 %; export.<br>Write the findings, including anything awkward, as a short list at the end of this file under "## Results of 0002.6". | 0002.5f | All 40 exports exist, none has a white corner, odd and even pages are each framed on their own page, pages 30–40 share one frame, text in the exports is clearly darker than with brightness applied the old way, and nothing in the photo folder changed except `pagecrop.json` and the one photo now in the trash. |

## Deferred

Not in this phase. Each can be added later without changing what is built here.

- Finding the page edges automatically for each side.
- A "swap sides from here" button for a photo that is missing from an alternating run.
- Restoring a deleted photo from inside the app. The desktop trash does this today.
- Keeping the undo history across a delete.
- Growing the frame back after nudging the angle with `[` and `]`.
- A shortcut for the alternating toggle.

## Risks worth naming before starting

- **Old settings files change meaning for brightness.** A folder saved with brightness +40 looks
  different after 0002.1: midtones lift less, and ink lifts less than before. Nothing converts old
  values. Signal: someone reopens a finished folder and the pages look darker. If that matters, add a
  `version: 2` bump in `persist.rs` and convert. Not planned.

- **Gamma still lifts dark grey ink somewhat.** At +50, ink at level 30 goes to 74. The old formula gave
  94, and pure black stays at 0. If text still looks too grey on real photos, the better tool is a
  white point, a gain that pushes the paper to white and moves ink only a little. It would be a new
  slider, not a change to this one. Ask the user before building it.

- **Deleting a page inside an alternating run shifts the sides of every later page.** That repairs a
  double shot (the common case). If the user deletes a real page they do not want, for example a blank
  one, every later page then has the wrong frame. The fix today: turn alternating off and on again on
  the right page. The confirmation dialog does not warn about it. If this bites, the deferred "swap
  sides from here" button is the answer.

- **Deleting the first page of a run when the next page has its own settings drops the deleted page's
  side.** Side A then falls back to the new anchor, which is the old side-B key. This is rare: it needs
  a deleted photo that is both the anchor and followed by a key. Check it once by hand in 0002.5f and
  write down what happened.

- **The trash may not work on every disk.** On a USB stick or network share, the trash crate uses a
  `.Trash-<uid>` folder at the top of that disk, or fails. A failure shows in the status bar and
  changes nothing, which is safe. Signal: "cannot move … to the trash" on a removable disk. Do not
  "fix" it with a permanent delete. Ask the user.

- **The turn direction of `fit_inside` can be flipped without any symmetric test noticing.** That is
  why `fitted_render_has_no_white_corners` uses an off-centre frame and the real renderer. Do not
  replace it with a centred frame because "it is simpler".

- **`effective` is now O(pages) per call, and export and the page list call it per page.** For 1 000
  pages that is about a million cheap steps per full pass, which is still well under a frame. Signal:
  the page list stutters while scrolling a large folder. Then cache `side` per frame. Do not do it in
  advance.

- **eframe persistence also stores window size and egui state** in `~/.local/share/pagecrop/`. That is
  harmless and expected. It is not the photo folder, so the `CLAUDE.md` rule holds.

- **API names from memory.** `trash::delete`, `Painter::with_clip_rect`, `ui.toggle_value` and the
  persistence functions in S6 were checked against the installed eframe 0.36.2 source where
  marked; the rest were not. The usual rule applies: read the docs of the installed version, do not
  guess and do not downgrade.
