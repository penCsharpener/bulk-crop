//! Thin terminal commands that call the library crates, for testing without a window.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

/// Command-line front end for pagecrop. Commands are added as the core functions they call exist.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::GenFixtures {
            dir,
            count,
            width,
            height,
            drift,
        } => {
            let paths = pagecrop_testkit::write_book(&dir, count, width, height, drift)
                .with_context(|| format!("writing pages to {}", dir.display()))?;
            println!("wrote {} pages to {}", paths.len(), dir.display());
        }
    }
    Ok(())
}
