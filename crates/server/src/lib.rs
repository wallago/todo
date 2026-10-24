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

use crate::config::Config;

/// Runs `server`.
///
/// # Errors
///
/// Returns an [`Error`] if the config can't be loaded or the server can't bind.
pub async fn run(args: &Args) -> Result<()> {
    better_panic::install();
    let config = Config::load(args.config.as_deref())?;
    server::start(&config.server.address, config.server.port).await?;
    tracing::info!("Server stopped");
    Ok(())
}
