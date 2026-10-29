//! Terminal interface for todo app.

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
/// Terminal user interface.
mod tui;

use prelude::*;

use crate::config::Config;

/// Runs `todo-tui`.
///
/// # Errors
///
/// Returns an [`Error`] if the run fa
pub async fn run(args: &Args) -> Result<()> {
    better_panic::install();
    let config = Config::load(args.config.as_deref())?;
    tracing::debug!("Loaded {config:?}");


    // Runs until Ctrl-C or until any component stops.
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            tracing::info!("Interrupted: {:?}", result);
        },
        result = tui::run() => {
            tracing::info!("TUI stopped: {:?}", result);
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn example_config_loads() -> Result<()> {
        // Only written when an option adds settings.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
        if path.exists() {
            Config::from_file(&path)?;
        }
        Ok(())
    }
}
