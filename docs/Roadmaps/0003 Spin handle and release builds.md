## Phase 0003 — Spin handle and release builds (P2)

Two requests after phase 0002:

1. **Turning the picture should not need the slider.** The fine-angle slider sits in the right panel,
   far from the picture, and one pixel of slider is about 0.3°. Paint.net turns a layer with a handle
   next to it. pagecrop gets the same: a round **spin handle** just outside the left and the right edge
   of the crop frame. Dragging it up or down turns the picture, in steps of 0.1°.
2. **Releases should build themselves.** Pushing a tag such as `v0.2.0` builds the Linux `.deb` and the
   Windows `.exe` on GitHub Actions and attaches them to a GitHub release.

Read `CLAUDE.md`, `docs/Roadmaps/readme.md` and the 0002 roadmap's sections "How the frame stays inside a
turned picture" and snippet S2 first.

## The model and decisions

### Spin handle

- There are two spin handles, round dots of radius 5 points in `ACCENT` with a 1-point dark outline. Each
  sits at the frame's vertical middle, outside the left and the right edge. A thin line joins each dot
  to the edge.
- The distance from the edge is `max(28, reach_x + 14)` screen points, where `reach_x` is the handle reach
  of 0002.3 in points. Keeping the dot beyond the reach zone means it never overlaps the E or W handle.
  When that position falls outside the canvas area, the dot is pulled back inside it, 10 points from the
  canvas edge.
- The pointer is on a spin handle when it is within 10 points of the dot. Spin handles are checked
  **before** the frame's `hit_test`.
- While the pointer is on a spin handle or spinning, the cursor is `Grab`, and `Grabbing` while
  dragging.
- The angle change is the angle the pointer sweeps around the **picture centre**, which is the pivot
  the picture turns about. On the right-hand dot, dragging down turns the picture clockwise. On the
  left-hand dot, dragging up turns it clockwise. That is the same sense as the slider, where positive
  means clockwise.
- The angle is rounded to 0.1° and clamped to ±15°, the slider's range.
- The frame is fitted with `App::angled`, the same mechanism as the slider drag. `angle_start` is set
  when the spin drag starts and cleared when it stops, so turning out and back within one drag gives the
  frame back.
- One spin drag is one undo step: `begin_edit` on press, `apply_edit` while dragging, `commit_edit` on
  release.
- Alternating sides need nothing new: the drag edits `effective()`, which is already the page's side.
- Space+drag still pans, and the middle button still pans. A spin drag never starts while Space is held.

The math goes in `crates/core/src/crop_edit.rs`, because it can be tested without a window:

```rust
/// The fine angle after dragging a spin handle from `press` to `now` (screen points), turning about
/// `centre`. The change is the angle the pointer swept around the centre: on the right of the centre,
/// moving down turns clockwise, which is positive. The result is rounded to 0.1° and kept within
/// `-limit..=limit`.
pub fn spin_angle(
    start_deg: f32,
    centre: (f32, f32),
    press: (f32, f32),
    now: (f32, f32),
    limit: f32,
) -> f32 {
    let before = (press.1 - centre.1).atan2(press.0 - centre.0);
    let after = (now.1 - centre.1).atan2(now.0 - centre.0);
    let mut swept = (after - before).to_degrees();
    // atan2 jumps by 360° where the pointer crosses the left of the centre.
    if swept > 180.0 {
        swept -= 360.0;
    } else if swept < -180.0 {
        swept += 360.0;
    }
    let angle = ((start_deg + swept) * 10.0).round() / 10.0;
    angle.clamp(-limit, limit)
}
```

Worked values for the tests, with the centre at (0, 0):
- press (100, 0), now (100, 10): the sweep is atan(0.1) = 5.71°, so the result is 5.7.
- press (−100, 0), now (−100, −10): the sweep crosses atan2's jump and gives +5.7 too.

The positions of the dots are screen layout, so they stay in `editor.rs`.

### Release workflow

- `.github/workflows/release.yml`, triggered by `push: tags: ['v*']`. Git tags do not belong to a branch;
  every tag pushed to the repository triggers it.
- Job `check` (ubuntu): the three gates from `CLAUDE.md`. It also checks that the tag without its `v`
  equals `version` in the root `Cargo.toml`, and fails with a clear message when not.
- Job `deb` (needs `check`, `ubuntu-22.04`, chosen for its older glibc so the package installs on more
  systems): install the system `-dev` packages the build needs, `cargo install cargo-deb --locked`, then
  `cargo deb -p pagecrop`. `cargo sweep` is not needed on a fresh runner. Upload
  `target/debian/*.deb`.
- Job `exe` (needs `check`, `windows-latest`): `cargo build --release -p pagecrop -p pagecrop-cli`.
  Upload `pagecrop-<version>-windows-x86_64.zip` containing `pagecrop.exe`, `pagecrop-cli.exe`,
  `LICENSE` and `README.md`, and also the bare `pagecrop.exe` renamed to
  `pagecrop-<version>-windows-x86_64.exe`.
- Job `release` (needs `deb` and `exe`): download both artifacts and create the GitHub release for the
  tag with them attached, using `softprops/action-gh-release` with generated release notes. Set
  `permissions: contents: write` on this job only.
- Use `Swatinem/rust-cache` in the build jobs, `actions/checkout`, `actions/upload-artifact` and
  `actions/download-artifact`. Use the current major version of each action. Pin majors (`@v4`), not
  SHAs.
- Windows: `crates/app/src/main.rs` gets `#![cfg_attr(windows, windows_subsystem = "windows")]` so no
  console window opens with the app.

## Tasks

| # | Task | Depends on | Done when |
|---|------|-----------|-----------|
| ✅ 0003.1 | **Spin angle** (core). Skill: image-geometry.<br>`crop_edit.rs`: `spin_angle` exactly as above.<br>Tests: `right_handle_down_turns_clockwise` (5.7); `left_handle_up_turns_clockwise` (5.7); `crossing_the_left_does_not_jump` (press (−100, 1), now (−100, −1): the sweep is about +1.15°, so the result is 1.1, not something near ±360); `result_is_rounded_to_a_tenth` (the result times 10 is a whole number within 1e-3); `result_is_clamped` (press (100, 0), now (0, 100), limit 15 → 15); `no_movement_keeps_start` (start 2.3 → 2.3). | — | `cargo test -p pagecrop-core crop_edit` is green with the six new tests. |
| 0003.1f | **Spin handles in the editor** (screen). Skills: egui-app, image-geometry.<br>`editor.rs`: a function giving the two dot positions on screen for a frame rect, the reach and the canvas area; painting of the dots and lines after the frame handles; and a spin-drag branch in `handle_pointer`, checked before the frame `hit_test` and following all the rules under "Spin handle". `app.rs`: a field `pub spin: Option<SpinDrag>` with `struct SpinDrag { start_deg: f32, press: egui::Pos2 }`. It is cleared wherever `drag` is cleared (delete, folder open). `view_input`'s "double-click on empty canvas" must not fire on a spin dot.<br>`help.rs`: add `("Drag a round handle beside the frame", "Turn the picture in 0.1° steps")`. README: one line in the workflow's straightening step. | 0003.1 | On `samples/book` page 6, dragging the right dot down by about the height of a text line turns the picture clockwise by a few tenths of a degree. The slider follows, showing tenths, the frame shrinks to stay inside, and dragging back up to the start gives the original frame back. The left dot works the other way round. Ctrl+Z undoes the whole spin in one step. The frame's E and W handles still work at 15 % reach. |
| 0003.2 | **Release workflow** (CI only, plus the `windows_subsystem` line).<br>Add `.github/workflows/release.yml` as described under "Release workflow". Check the Windows build compiles without a Windows machine: `rustup target add x86_64-pc-windows-gnu` and `cargo check --target x86_64-pc-windows-gnu -p pagecrop -p pagecrop-cli`. Fix anything Linux-only that breaks it, behind `cfg` where needed and keeping Linux behaviour unchanged. Validate the YAML with `actionlint` if it can be installed (`go install` or the release binary); otherwise at least parse it with Python's `yaml`. Add a "Releases" paragraph to the README: bump `version` in the root `Cargo.toml`, commit, `git tag v<version>`, `git push origin v<version>`. | 0003.1f | The YAML parses (and passes actionlint if available). `cargo check --target x86_64-pc-windows-gnu` succeeds for both binaries. The three gates are clean. The first real run happens when the user pushes a tag. |

## Risks

- **The Windows build was never run.** `cargo check` for the GNU target catches compile errors, not
  run-time ones. Paths, the trash crate's Windows backend and the file dialog are untested on Windows.
  The roadmap 0001 decision "Linux only" still holds for testing; the `.exe` is a convenience.
- **ubuntu-22.04 runners will be retired by GitHub one day.** When that happens, the workflow fails to
  start. Move to the next LTS image; the `.deb` will then need a newer glibc.
- **The tag must match `Cargo.toml`.** The check job stops a release whose files would carry a different
  version than its tag.
