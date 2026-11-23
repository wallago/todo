//! Command line entry point for `server`.

use std::process::exit;

use clap::Parser;
use todo_server::{prelude::*, version};
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
    let under_journal = std::env::var_os("JOURNAL_STREAM").is_some();
    let fmt = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr);
    if under_journal {
        fmt.without_time().with_ansi(false).init();
    } else {
        fmt.init();
    }

    tracing::info!("welcome into {}", version!());
    match todo_server::run(&args).await {
        Ok(()) => exit(0),
        Err(e) => {
            eprintln!("{e}");
            exit(1)
        }
    }
}
