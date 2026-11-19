//! Command line entry point for `todo_tui`.

use std::process::exit;

use clap::Parser;
use todo_tui::{prelude::*, version};
use tracing::Level;

/// Runs the crate and maps the outcome to a process exit code.
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let level = match args.verbose {
        0 => Level::WARN, // default: warnings + errors only
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE, // -vvv and beyond
    };
    // The TUI owns the terminal, so logs go to a file instead of stderr.
    match log_file() {
        Ok(file) => tracing_subscriber::fmt()
            .with_max_level(level)
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .init(),
        Err(e) => eprintln!("logging disabled: {e}"),
    }

    tracing::info!("welcome into {}", version!());
    match todo_tui::run(&args).await {
        Ok(()) => exit(0),
        Err(e) => {
            eprintln!("{e}");
            exit(1)
        }
    }
}

/// Opens (appending) `<state dir>/<crate>/<crate>.log`, creating the folder.
///
/// # Errors
///
/// Returns an error if the folder or the file can't be created.
fn log_file() -> std::io::Result<std::fs::File> {
    let dir = dirs::state_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join(env!("CARGO_PKG_NAME"));
    std::fs::create_dir_all(&dir)?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(concat!(env!("CARGO_PKG_NAME"), ".log")))
}
