//! Just a personal Todo app.

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

/// Runs `todo`.
///
/// # Errors
///
/// Returns an [`Error`] if the run fa
pub async fn run(args: &Args) -> Result<()> {
    better_panic::install();
    let _config = Config::load(args.config.as_deref())?;


    tokio::select! {
        result = server::start(&config.server.address, config.server.port, pdb_rx, seat_rx) => {
            tracing::info!("Server stopped: {:?}", result);
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn run_succeeds() {
        assert_eq!(run(Args::default()).is_ok(), true);
    }
}
