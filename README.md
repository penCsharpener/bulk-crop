# pagecrop

pagecrop crops, straightens and shrinks photos of book pages in bulk. Photographing a book page by
page makes the book drift: each photo is turned a little and slides a little. pagecrop lets you fix
one page, and the same fix then applies to every following page until a page that has its own
setting. Nothing is written to your photos. The result is a separate folder of corrected pictures.

It runs on Linux, as a desktop window written in Rust with egui.

## Build and install

You need a stable Rust toolchain (rustup) and a C linker. If the build names a missing system
library, install its `-dev` package and build again.

From the root of this repository:

```sh
cargo install --path crates/app
```

This puts the program `pagecrop` in `~/.cargo/bin`. Open a folder of photos from any directory:

```sh
pagecrop ~/some/photo/folder
```

To add it to the application menu, copy the desktop entry to `~/.local/share/applications`, and the
icon to the icon theme:

```sh
cp assets/pagecrop.desktop ~/.local/share/applications/
mkdir -p ~/.local/share/icons/hicolor/64x64/apps
cp assets/pagecrop.png ~/.local/share/icons/hicolor/64x64/apps/
```

## Workflow

1. **Open the folder.** Click Open folder…, press Ctrl+O, or start the program with the folder as its
   argument. Every photo appears in the page list on the left.
2. **Choose the sort.** Name or Date. Pick the order the pages were photographed in, because a page's
   settings apply to the pages after it in this order.
3. **Go to the first page that needs fixing.** Use the arrow keys, or click a page in the list.
4. **Set the frame, the turn, the angle and the tone.** Drag the handles of the frame to crop. Turn with
   R (Shift+R turns the other way) or the ⟲ and ⟳ buttons. Set the fine angle with [ and ] or the
   slider. Brightness, contrast and greyscale are in the Tone section. The page now has its own
   settings, shown by a dot in the page list. Later pages follow it until the next page with its own
   settings. Press N to jump to that page.
5. **Check and correct.** Press P to see the result as it will be exported. Ctrl+Z takes back the last
   change, and Ctrl+Shift+Z or Ctrl+Y brings it back. Delete resets the page to the settings before it.
6. **Export.** Click Export… or press Ctrl+E. The output folder defaults to a `cropped` folder inside
   the photo folder. Choose the long edge (1600 pixels is the default), or keep the cropped size, or
   shrink by a percentage. Quality is 90 by default. Click Start, and watch the progress. Cancel stops
   the export; pictures already written stay. Existing files are skipped unless you tick Replace
   existing files.

## Shortcuts

| Keys | What it does |
|---|---|
| Right, Down, Page Down | Next page |
| Left, Up, Page Up | Previous page |
| Home, End | First page, last page |
| N, Shift+N | Next, previous page with its own settings |
| R, Shift+R | Turn 90° clockwise, anticlockwise |
| [ and ] | Fine angle −0.1°, +0.1° |
| Shift+[ and Shift+] | Fine angle −1°, +1° |
| Delete | Reset the page to the settings before it |
| G | Show or hide the grid |
| P | Show the result, or the frame again |
| 0 | Fit the picture to the window |
| Mouse wheel | Zoom in or out around the pointer |
| Middle drag, or Space + drag | Pan the zoomed picture |
| Double click on empty picture | Fit the picture to the window |
| Drag a handle of the frame | Move that edge or corner |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z, Ctrl+Y | Redo |
| Ctrl+O | Open a folder |
| Ctrl+E | Show or hide the export window |
| F1 | Show or hide the list of shortcuts |

The shortcuts do nothing while a text field has focus, so you can type in the output folder field.

## The settings file: pagecrop.json

The settings live in `pagecrop.json`, in the photo folder. It is written when you change a page, and
under a temporary name first, so an interrupted save does not damage it. It holds:

- `version`: the file format, currently 1.
- `sort`: the order of the pages, Name or Date.
- `keys`: one entry for each page that has its own settings, named by the photo's file name. Each entry
  has the turn (`quarter_turns`), the fine angle (`angle_deg`), the frame (`crop` with `cx`, `cy`, `w`
  and `h` as fractions of the picture), `brightness`, `contrast` and `grayscale`.
- `export`: the output folder and the export options last used.

Settings for photos that are no longer in the folder are kept in the file, so removing a photo does
not lose them. A renamed photo does not get its settings back, because they are stored by file name.
Your photos are never changed. pagecrop writes only `pagecrop.json` in the photo folder, and the
pictures into the export folder, which it creates only when you start an export.

## Deferred features

These are not in this version. Each can be added later.

- Lossless crop for pages that need neither a fine angle nor shrinking.
- Running in a browser.
- Two crop frames per photo, for double-page spreads.
- Four-corner perspective correction.
- Windows and macOS builds.
- Copying EXIF data, or setting the print resolution in the output.
- Leaving single photos out of the export.
- Thumbnails that show the cropped result instead of the whole photo.
- Finding the page edges and the angle automatically.

## Known limits

- Where a frame reaches outside the photo, the outside part is exported white.
- A picture that is both turned and shrunk is resampled twice, so the output can look slightly softer
  than a single pass would give.
- Speed: on the machine used for the measurement, about 2.7 photos per second, measured on synthetic
  photos. See `docs/performance.md`.
