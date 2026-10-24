## Phase 0004 — Remembered folder, tone presets, bigger spin handle (P2)

Three requests after phase 0003:

1. **Start where I stopped.** The app opens empty unless a folder is given on the command line. It
   should reopen the folder that was open when it was closed.
2. **Tone is two sliders every time.** Brightness and contrast are dialled in again for each book.
   The app keeps the last three distinct tone settings as buttons; one click puts a setting back.
3. **The spin handle is hard to hit.** Its grab area is 10 points. It should be as generous as the
   frame handles, and follow the same "Handle reach" setting.

Read `CLAUDE.md` and `docs/Roadmaps/readme.md` first. Phase 0003's section "Spin handle" is changed by
row 0004.3.

## The model and decisions

### Remembered folder

- App-wide, kept in eframe's storage next to `handle_reach_percent`, under the key `last_folder`, as a
  string. It is not written into any photo folder.
- It is written by `eframe::App::save` from the open project's `dir`. While no folder is open, the
  stored value is left alone.
- On start, a folder given on the command line wins. Otherwise the stored folder is opened when it is
  still a directory; if it is gone, the app starts empty and says nothing.

### Tone presets

- `pagecrop_core::tone::ToneSetting { brightness: i32, contrast: i32, grayscale: bool }`, the three
  tone fields of an `Adjust`. `ToneSetting::of(&Adjust)` reads them, `ToneSetting::applied_to(self,
  Adjust) -> Adjust` writes them. The default is the default tone (0, 0, false).
- `pagecrop_core::tone::remember(list, tone, replacing) -> Vec<ToneSetting>` is the list rule, at most
  `TONE_MEMORY` (3) entries, newest first:
  1. `replacing` is taken out of the list first.
  2. The default tone is never stored (the Reset tone button covers it).
  3. A tone that is already in the list keeps its place, so the buttons do not jump about under the
     pointer.
  4. Any other tone goes to the front and the list is cut to three.
- Setting brightness and then contrast is two edits, and the first would leave a half-finished entry
  in a three-slot list. So the app remembers which entry the current run of tone edits created
  (`App::tone_run`) and passes it as `replacing`. `pagecrop_core::tone::record(list, run, tone) ->
  (Vec<ToneSetting>, Option<ToneSetting>)` is `remember` plus the new owner of the run: `tone` when
  this call put it in the list, else none. A run ends when another page is selected, a folder
  is opened, or a preset or Reset tone is clicked.
- The list is remembered by the app between runs, in eframe's storage under `tone_presets`. It is
  app-wide, not per folder, because the same tone is wanted for the next book.
- The Tone section shows the presets as buttons labelled `+20 / −35` (brightness / contrast, `grey`
  added for greyscale), the one equal to the page's tone drawn selected. Clicking one is one undo step.
  The edit that records a tone is whichever edit of a page changes its effective tone; undo and redo
  do not record.

### Spin handle reach

- `pagecrop_core::crop_edit::spin_reach(base_points, percent, floor) -> f32`: `percent` per cent of the
  base picture's shorter side on screen, at least `floor`. This is the same distance the frame handles
  reach before their cap at a third of the frame.
- The spin handle's grab area is a circle of that radius (floor 10 points, the old fixed value) around
  the dot. The dot is still drawn at radius 5.
- The dot sits `max(28, reach_x + radius + 4)` points from the frame edge, so the two zones just touch
  without overlapping the E or W handle's zone. Phase 0003 had `reach_x + 14`, which is the same rule
  for a 10-point radius.

## Tasks

| # | Task | Depends on | Done when |
|---|------|-----------|-----------|
| ✅ 0004.1 | **Remember the last folder** (screen only). Skill: egui-app.<br>`app.rs`: `fn startup_folder(arg: Option<PathBuf>, stored: Option<String>) -> Option<PathBuf>` as described under "Remembered folder"; `App::new` reads the key `last_folder` and opens what it returns; `save` writes it.<br>Tests: `argument_wins_over_stored_folder`; `stored_folder_is_used_when_it_exists` (use `env!("CARGO_MANIFEST_DIR")`); `missing_stored_folder_is_ignored`; `nothing_given_gives_nothing`.<br>README: one sentence in workflow step 1. | — | Start the app with `samples/book`, close it, start it with no argument: the same folder opens. Open another folder, close, start again: that one opens. A stored folder that was renamed gives an empty window and no message. |
| ✅ 0004.2 | **Tone presets** (core).<br>`tone.rs`: `ToneSetting`, `ToneSetting::of`, `ToneSetting::applied_to`, `TONE_MEMORY`, `remember` and `record` as described under "Tone presets".<br>Tests: `new_tone_goes_to_the_front`; `list_is_cut_to_three`; `known_tone_keeps_its_place`; `default_tone_is_not_stored`; `replacing_entry_is_swapped_for_the_new_one`; `replacing_an_unlisted_tone_changes_nothing_else`; `two_edits_of_a_run_leave_one_entry`; `a_new_run_adds_a_second_entry`; `a_listed_tone_is_not_owned_by_the_run`; `the_default_tone_ends_the_ownership`; `of_and_applied_to_round_trip` (other fields of the `Adjust` untouched). | — | `cargo test -p pagecrop-core tone` is green with the eleven new tests. |
| ✅ 0004.2f | **Tone presets in the Tone section** (screen). Skill: egui-app.<br>`app.rs`: fields `pub tone_presets: Vec<ToneSetting>`, `tone_run: Option<ToneSetting>` and `edit_tone: ToneSetting` (the page's tone when the edit began); `commit_edit` records a tone change as above; `select` (when the page changes) and `install` end the run; `pub fn use_tone_preset(&mut self, tone)`; `new`/`save` keep the list under `tone_presets`.<br>`panels/adjust.rs`: the buttons above "Reset tone", wrapping, with a hover text naming brightness, contrast and greyscale.<br>README: one sentence in workflow step 4. | 0004.2 | Set brightness +30 and contrast +20 on one page: one button `+30 / +20` appears (not two). Go to another page, set −10 / 0 and then 40 / 10 / grey: three buttons, newest first. A fourth different tone pushes the oldest out. Clicking a button sets both sliders and the checkbox at once, does not reorder the buttons, and Ctrl+Z undoes it. After restarting the app the buttons are still there. |
| 0004.3 | **Spin handle follows the handle reach** (core and screen). Skills: egui-app, image-geometry.<br>`crop_edit.rs`: `spin_reach` as above. Tests: `spin_reach_is_a_share_of_the_shorter_side` (5 % of (800, 600) is 30); `spin_reach_never_below_the_floor` (1 % of (800, 600) with floor 10 is 10).<br>`editor.rs`: `SPIN_REACH` becomes the floor; `spin_dots` takes the radius and uses `max(28, reach_x + radius + 4)`; the hit tests use the radius. Update its three tests and add `dots_move_out_with_the_grab_radius`.<br>`app.rs`/`adjust.rs`: the "Handle reach" hover text also names the spin handles. `README.md` and the 0003 text under "Spin handle" say the new rule. | ✅ 0004.2f | With "Handle reach" at 5 %, the spin dot reacts from a clearly larger distance than before (about the width of the E/W handle zone); at 15 % it is larger still; at 1 % it is 10 points as before. The E and W handles still work right up to the edge of the spin zone, and a drag started on the spin zone still turns the picture. |

## Risks

- **Presets are per machine, not per folder.** They are not written into `pagecrop.json`, so they do
  not travel with the photos. That is on purpose: a tone is a habit of the user, not a property of one
  book.
- **A large reach pushes the dots far from the frame.** At 15 % the dot can sit over 200 points away,
  and near the canvas edge it is pulled back, where its zone may overlap the E or W handle's. The spin
  handle wins there because it is checked first.
