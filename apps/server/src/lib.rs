//! Server todo app.

/// Main application.
pub mod app;
/// Command-line arguments parser.
pub mod args;
/// Config file.
pub mod config;
/// Error handler implementation.
pub mod error;
/// Helper functions.
pub mod help;
/// Common types that can be glob-imported for convenience.
pub mod prelude;
/// Web layer.
pub mod server;

use prelude::*;

use crate::app::AppState;
use crate::config::Config;

/// Runs `server`.
///
/// # Errors
///
/// Returns an [`Error`] if the config, database or token can't be loaded,
/// or the server can't bind.
pub async fn run(args: &Args) -> Result<()> {
    better_panic::install();
    if let Some(Command::Import { vault, db }) = &args.command {
        return import(args, vault, db.as_deref());
    }
    let config = Config::load(args.config.as_deref())?;
    let state = AppState::open(&config.database.path)?;
    server::start(&config, state).await?;
    tracing::info!("Server stopped");
    Ok(())
}

/// `todo-server import`: copies a vault into the database.
#[expect(clippy::print_stdout, reason = "the report is the command's output")]
fn import(args: &Args, vault: &std::path::Path, db: Option<&std::path::Path>) -> Result<()> {
    let path = match db {
        Some(path) => path.to_owned(),
        None => Config::load(args.config.as_deref())?.database.path,
    };
    let report = todo_store::Store::open(&path)?.import_vault(vault)?;
    println!(
        "{}: {} topics read, {} todos added, {} already there",
        path.display(),
        report.topics,
        report.added,
        report.skipped
    );
    Ok(())
}
