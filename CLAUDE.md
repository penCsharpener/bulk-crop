# pagecrop

Desktop tool (Linux, Rust, egui) that crops, straightens and shrinks photos of book pages in bulk.
An adjustment made on one page applies to every following page until the next page that has its own
adjustment.

The work is planned in `docs/Roadmaps/`. Work one table row at a time, top to bottom. Before starting a
row, load the `roadmap-row` skill; it is the procedure for every row.

## Layout

| Path | Crate | Holds | Must not hold |
|---|---|---|---|
| `crates/core` | `pagecrop-core` (lib) | model, geometry, decode, render, tone, export, persistence | any `egui`, `eframe`, `rfd`, `clap` import |
| `crates/cli` | `pagecrop-cli` (bin) | thin commands that call core, for testing from a terminal | image math |
| `crates/app` | `pagecrop` (bin) | egui windows, input handling, background loading | image math, file formats |
| `crates/testkit` | `pagecrop-testkit` (lib) | synthetic page images for tests and manual runs | anything from core |

If a calculation could be unit-tested without a window, it belongs in `crates/core`.

## The three gates

Run all three before calling a row done. Fix every finding; do not silence one.

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

## Rust rules for this repo

**Errors**
- `crates/core` returns `Result<T, CoreError>`. `CoreError` is one `thiserror` enum in `error.rs`. Every
  variant that concerns a file carries its `PathBuf`.
- The two binaries use `anyhow::Result` and add context with `.with_context(|| format!(...))`.
- No `unwrap()`, `expect()`, `panic!`, `todo!` or slice indexing that can go out of range outside tests.
  A bad file must produce an `Err`, never a crash.
- No `unsafe`.

**Types and signatures**
- Parameters borrow: `&Path`, `&str`, `&[T]`, `&RgbImage`. Return values are owned.
- Data types derive `Debug, Clone, PartialEq`. Small plain values also derive `Copy`.
- Anything written to disk derives `Serialize, Deserialize` and has `#[serde(default)]` on the struct, so
  older files keep loading when a field is added, so a settings file copied from an older version loads
  what still fits. Each minor version has its own settings file; there is no migration.
- Convert numbers on purpose. Round or clamp first, then cast: `(x.round().max(1.0)) as u32`. Never cast
  a possibly negative float straight to an unsigned integer.
- Compare floats in tests with a tolerance (`(a - b).abs() < 1e-4`), never with `==`.

**Structure**
- Functions stay small and do one thing. Prefer pure functions (inputs in, value out, no I/O).
- One module per topic, named in the roadmap. Do not create extra modules, traits or generics the row
  does not ask for.
- Do not change the signature of a function an earlier row defined. If it really has to change, update
  every caller and the roadmap text in the same commit.

**Dependencies**
- Add a crate with `cargo add <name> -p <crate>`. Never type a version number from memory.
- Commit `Cargo.lock`.
- If a function named in the roadmap does not exist in the installed version, open the docs for that
  exact version (`cargo doc -p <crate> --open`, or docs.rs) and use the current name. Do not invent an
  API, and do not downgrade the crate.

**Tests**
- Unit tests sit at the bottom of the file they test, in `#[cfg(test)] mod tests`.
- Tests that touch the disk use the `tempfile` crate. No test reads a file outside the repo, and none
  needs the network.
- Test images are built in memory or with `pagecrop-testkit`. Compare pixels on in-memory images, not on
  JPEG files, because JPEG changes pixel values.
- Write the test named in the row before the code that makes it pass.

**Threads and the UI**
- The UI thread never reads a file, decodes an image or renders at full size. That work runs on a
  worker and comes back over a channel.
- Always measure and demo with optimisation on. The workspace `Cargo.toml` sets `opt-level` for dev
  builds; without it one preview takes seconds and the app looks broken.

## Data that must not be lost

- Original photos are never written to. The only exception is the bin button: after the user confirms, it moves one photo to the desktop trash with `delete::move_to_trash`. Nothing else may delete, move or change a photo.
- The app writes into the export folder and writes the settings file `persist::SIDECAR_NAME`
  (`pagecrop_v05.json` in 0.5) into the photo folder. It writes nothing else there, ever.
- The settings file `persist::SIDECAR_NAME` holds the user's adjustments. Save it by writing a temporary
  file and renaming it.

## Commits

One commit per roadmap row, message `0001.7f: crop frame in editor`. Commit only when the gates are
clean and the row's "Done when" has been checked by hand.
