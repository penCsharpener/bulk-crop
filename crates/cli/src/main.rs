//! Thin terminal commands that call the library crates, for testing without a window.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use pagecrop_core::decode::{format_taken, load_oriented, read_orientation};
use pagecrop_core::model::{Project, SortBy};

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
    /// List the pictures in a folder, one per line with its date, then the count.
    Scan {
        dir: PathBuf,
        #[arg(long, value_enum, default_value_t = SortArg::Name)]
        sort: SortArg,
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

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
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
                println!("{}  {}", format_taken(entry.taken), entry.file_name);
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
