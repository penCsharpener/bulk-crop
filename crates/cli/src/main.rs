//! Thin terminal commands that call `pagecrop-core`, for testing without a window.

use clap::Parser;

/// Command-line front end for pagecrop. Commands are added as the core functions they call exist.
#[derive(Parser)]
#[command(version)]
struct Cli {}

fn main() {
    let _ = Cli::parse();
}
