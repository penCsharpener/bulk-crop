---
name: image-io
description: How pagecrop reads, orients, shrinks, tone-adjusts and writes JPEG files with the image crate, and how tests get their pictures. Use for work in crates/core decode.rs, tone.rs, encode.rs, export.rs, scan.rs and in crates/testkit.
---

# Reading and writing images

Crate names below are correct. Function names are from the versions known when this was written. If one
does not compile, look it up in the docs of the installed version. Do not guess.

## Crates

| Need | Crate | Notes |
|---|---|---|
| Decode, resize, encode | `image` | Add with `--no-default-features --features jpeg`. |
| EXIF orientation and date | `kamadak-exif` | Imported as `exif`. |
| Natural file-name order | `natord` | `page_2` sorts before `page_10`. |
| Date parsing | `chrono` | `NaiveDateTime::parse_from_str`. |
| Parallel work | `rayon` | Export and thumbnails. |
| Temp folders in tests | `tempfile` | dev-dependency. |
| Setting file times in tests | `filetime` | dev-dependency. |

## Working pixel format

Everything between decode and encode is `image::RgbImage` (8 bits, 3 channels). Convert once after
decoding with `.to_rgb8()`. Only the very last step may turn it into a grey image.

## Decode

```rust
let img = image::ImageReader::open(path)?.decode()?.to_rgb8();
```

Map both errors to `CoreError` with the path. A file that is empty or not a JPEG must give an `Err`.

## EXIF orientation

Phones store portrait photos sideways and add a tag saying how to turn them. Read tag `Orientation`
(a number 1 to 8). Any failure to read it means 1.

| Value | What to do to the decoded image |
|---|---|
| 1 | nothing |
| 2 | `flip_horizontal` |
| 3 | `rotate180` |
| 4 | `flip_vertical` |
| 5 | `rotate90`, then `flip_horizontal` |
| 6 | `rotate90` |
| 7 | `rotate270`, then `flip_horizontal` |
| 8 | `rotate270` |

All four functions are in `image::imageops`. `rotate90` turns clockwise.

Apply the orientation directly after decoding, before any other step. Nothing downstream knows about
EXIF.

## Date taken

Read tag `DateTimeOriginal`. Its text looks like `2026:01:06 16:53:14`. Parse with the format
`%Y:%m:%d %H:%M:%S` and store seconds as `i64`. If the tag is missing or unreadable, use the file's
modification time.

## Shrinking

- Previews and thumbnails: `image::imageops::thumbnail(&img, w, h)`. Fast, good enough for the screen.
- Export: `image::imageops::resize(&img, w, h, FilterType::Lanczos3)`. Slow, sharp.
- Never enlarge. If the target is not smaller than the image, return the image unchanged.

## Tone

Brightness and contrast are integers from -100 to 100. Build a table of 256 values once per image:

```
k = 1.0 + contrast / 100.0                      (0.0 to 2.0)
out(v) = clamp(round((v - 127.5) * k + 127.5 + brightness * 1.275), 0, 255)
```

Apply the table to every channel of every pixel. Brightness 0 and contrast 0 give the identity table;
skip the pass in that case.

Greyscale uses `round(0.299 * R + 0.587 * G + 0.114 * B)`. Order: geometry, then greyscale, then the
table.

## Encode

```rust
let mut out = BufWriter::new(File::create(&tmp_path)?);
let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
img.write_with_encoder(enc)?;
```

Always write to `<name>.tmp` first, flush, then `std::fs::rename` to the final name. A cancelled or
crashed export then never leaves half a file.

A grey image is written as a one-channel JPEG (`DynamicImage::ImageLuma8`). It is much smaller.

## Listing a folder

- Only the folder itself, no subfolders.
- Keep files whose extension is `jpg` or `jpeg` in any letter case.
- Skip names starting with `.` and names ending in `.tmp`.

## Pictures for tests

Never commit photos. Build pictures in code:

- Tiny images with one distinct colour per pixel for orientation and mapping tests.
- `pagecrop_testkit::page_image(w, h, index, angle_deg)` for a tilted page on a dark table.
- `pagecrop_testkit::write_book(dir, count, w, h, drift)` for a folder of JPEG files.

Compare exact pixels only on in-memory images. After a JPEG round trip compare sizes, or compare
colours with a tolerance of about 12 per channel.

Real photos for manual checks go in `samples/real/`, which is ignored by git.
