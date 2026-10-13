//! Thin terminal commands that call the library crates, for testing without a window.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use pagecrop_core::decode::{format_taken, load_oriented, read_orientation};
use pagecrop_core::encode::write_jpeg;
use pagecrop_core::export::{ExportOptions, Outcome, Progress, plan, run};
use pagecrop_core::geometry::Resize;
use pagecrop_core::model::{Adjust, CropRect, Project, SortBy};
use pagecrop_core::render::render;

/// Command-line front end for pagecrop. Commands are added as the core functions they call exist.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Sort order for `scan`.
#[derive(Clone, Copy, ValueEnum)]
enum SortArg {
    Name,
    Date,
}

#[derive(Subcommand)]
enum Command {
    /// Print the size of a picture after its EXIF orientation is applied, and the orientation.
    Info { file: PathBuf },
    /// List the pictures in a folder, one per line with its date, then the count. A * in front of
    /// a name marks a page that has its own settings in pagecrop.json.
    Scan {
        dir: PathBuf,
        #[arg(long, value_enum, default_value_t = SortArg::Name)]
        sort: SortArg,
    },
    /// Export every picture with its settings into an output folder. Exits with code 2 when a
    /// picture could not be exported.
    Export {
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Shrink so the longer side is this many pixels at most.
        #[arg(long, conflicts_with_all = ["percent", "no_resize"])]
        long_edge: Option<u32>,
        /// Shrink to this percentage of the cropped size.
        #[arg(long, conflicts_with = "no_resize")]
        percent: Option<f32>,
        /// Keep the cropped size.
        #[arg(long)]
        no_resize: bool,
        /// JPEG quality, 1 to 100.
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        /// Replace files that already exist in the output folder.
        #[arg(long)]
        overwrite: bool,
    },
    /// Turn, crop and shrink one picture, and write it as a JPEG.
    Render {
        file: PathBuf,
        /// JPEG file to write; the original is never overwritten.
        #[arg(long)]
        out: PathBuf,
        /// Quarter turns clockwise, 0 to 3.
        #[arg(long, default_value_t = 0)]
        turns: u8,
        /// Fine angle in degrees; positive turns clockwise.
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        angle: f32,
        /// Frame as cx,cy,w,h in fractions of the picture.
        #[arg(long, default_value = "0,0,1,1", value_parser = parse_crop, allow_hyphen_values = true)]
        crop: CropRect,
        /// Shrink so the longer side is this many pixels at most.
        #[arg(long, conflicts_with = "percent")]
        long_edge: Option<u32>,
        /// Shrink to this percentage of the cropped size.
        #[arg(long, conflicts_with = "long_edge")]
        percent: Option<f32>,
        /// Brightness, -100 to 100.
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        brightness: i32,
        /// Contrast, -100 to 100.
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        contrast: i32,
        /// Write the picture in greyscale.
        #[arg(long)]
        gray: bool,
        /// JPEG quality, 1 to 100.
        #[arg(long, default_value_t = 90)]
        quality: u8,
    },
    /// Write a synthetic book of tilted pages into a folder, for testing.
    GenFixtures {
        /// Folder to write into; created if missing.
        dir: PathBuf,
        #[arg(long, default_value_t = 12)]
        count: u32,
        #[arg(long, default_value_t = 1600)]
        width: u32,
        #[arg(long, default_value_t = 1200)]
        height: u32,
        /// Degrees each page is turned further clockwise than the one before.
        #[arg(long, default_value_t = 0.4, allow_negative_numbers = true)]
        drift: f32,
        /// Give later pages older modification times, so the last page is the oldest.
        #[arg(long)]
        reverse_mtime: bool,
    },
}

/// Reads a crop frame written as `cx,cy,w,h`.
fn parse_crop(text: &str) -> Result<CropRect, String> {
    let values = text
        .split(',')
        .map(|v| match v.trim().parse::<f32>() {
            Ok(n) if n.is_finite() => Ok(n),
            Ok(_) => Err(format!("{} is not a finite number", v.trim())),
            Err(err) => Err(err.to_string()),
        })
        .collect::<Result<Vec<f32>, String>>()?;
    match values.as_slice() {
        [cx, cy, w, h] => Ok(CropRect {
            cx: *cx,
            cy: *cy,
            w: *w,
            h: *h,
        }),
        _ => Err("expected four numbers: cx,cy,w,h".to_owned()),
    }
}

/// Whether two paths name the same file, so a render never overwrites its own input.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Export {
            dir,
            out,
            long_edge,
            percent,
            no_resize,
            quality,
            overwrite,
        } => {
            let project =
                Project::open(&dir).with_context(|| format!("opening {}", dir.display()))?;
            let resize = match (long_edge, percent, no_resize) {
                (Some(n), _, _) => Resize::LongEdge(n),
                (None, Some(p), _) => Resize::Percent(p),
                (None, None, true) => Resize::None,
                (None, None, false) => ExportOptions::default().resize,
            };
            let opts = ExportOptions {
                out_dir: out.clone(),
                resize,
                quality,
                overwrite,
            };
            let jobs = plan(&project, &opts)
                .with_context(|| format!("planning the export to {}", out.display()))?;
            let failed = AtomicUsize::new(0);
            run(
                &jobs,
                &opts,
                &AtomicBool::new(false),
                &|progress| match progress {
                    Progress::Item { index, outcome } => {
                        let name = project
                            .entries
                            .get(index)
                            .map_or("?", |e| e.file_name.as_str());
                        match outcome {
                            Outcome::Written => println!("{name} written"),
                            Outcome::Skipped => println!("{name} skipped"),
                            Outcome::Failed(reason) => println!("{name} failed: {reason}"),
                        }
                    }
                    Progress::Finished {
                        written,
                        skipped,
                        failed: count,
                        ..
                    } => {
                        failed.store(count, Ordering::Relaxed);
                        println!("{written} written, {skipped} skipped, {count} failed");
                    }
                    Progress::Started { .. } => {}
                },
            )
            .context("exporting the pictures")?;
            if failed.load(Ordering::Relaxed) > 0 {
                std::process::exit(2);
            }
        }
        Command::Render {
            file,
            out,
            turns,
            angle,
            crop,
            long_edge,
            percent,
            brightness,
            contrast,
            gray,
            quality,
        } => {
            anyhow::ensure!(
                !same_file(&file, &out),
                "--out must not be the input file {}",
                file.display()
            );
            let src =
                load_oriented(&file).with_context(|| format!("reading {}", file.display()))?;
            let adjust = Adjust {
                quarter_turns: turns,
                angle_deg: angle,
                crop,
                brightness,
                contrast,
                grayscale: gray,
            }
            .normalized();
            let resize = match (long_edge, percent) {
                (Some(n), _) => Resize::LongEdge(n),
                (None, Some(p)) => Resize::Percent(p),
                (None, None) => Resize::None,
            };
            let img = render(&src, &adjust, resize);
            let (w, h) = (img.width(), img.height());
            write_jpeg(&out, &img, quality)
                .with_context(|| format!("writing {}", out.display()))?;
            println!("wrote {w}x{h} to {}", out.display());
        }
        Command::Info { file } => {
            let img =
                load_oriented(&file).with_context(|| format!("reading {}", file.display()))?;
            let (w, h) = img.dimensions();
            println!("{w}x{h} orientation={}", read_orientation(&file));
        }
        Command::Scan { dir, sort } => {
            let mut project =
                Project::open(&dir).with_context(|| format!("scanning {}", dir.display()))?;
            project.sort(match sort {
                SortArg::Name => SortBy::Name,
                SortArg::Date => SortBy::Date,
            });
            for entry in &project.entries {
                let mark = if entry.key.is_some() { "*" } else { " " };
                println!("{}  {mark}{}", format_taken(entry.taken), entry.file_name);
            }
            println!("{} images", project.entries.len());
        }
        Command::GenFixtures {
            dir,
            count,
            width,
            height,
            drift,
            reverse_mtime,
        } => {
            let paths =
                pagecrop_testkit::write_book(&dir, count, width, height, drift, reverse_mtime)
                    .with_context(|| format!("writing pages to {}", dir.display()))?;
            println!("wrote {} pages to {}", paths.len(), dir.display());
        }
    }
    Ok(())
}
