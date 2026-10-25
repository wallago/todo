//! Config file setup.

use std::path::Path;

use serde::Deserialize;

use crate::prelude::*;

/// Server settings.
pub(crate) mod server;
/// Vault settings.
pub(crate) mod vault;

/// Configuration loaded from `config.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Server config.
    pub(crate) server: server::ServerConfig,
    /// Vault config.
    pub(crate) vault: server::ServerConfig,
}

impl Config {
    /// Loads the config: explicit `--config` path, else
    /// `$XDG_CONFIG_HOME/env!("CARGO_PKG_NAME")/config.toml` if present, else defaults.
    ///  
    /// # Errors
    ///
    /// Returns an error if a config file is found but cannot be read or parsed.
    pub(crate) fn load(cli_path: Option<&Path>) -> Result<Self> {
        if let Some(path) = cli_path {
            return Self::from_file(path);
        }
        match dirs::config_dir()
            .map(|dir| dir.join(format!("{}/config.toml", env!("CARGO_PKG_NAME"))))
        {
            Some(path) if path.exists() => Self::from_file(&path),
            _ => Err(Error::Config("file not found".to_string())),
        }
    }

    /// Reads and parses a config file; any failure is a hard error.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, or if its contents are not
    /// valid TOML for this config.
    pub(crate) fn from_file(path: &Path) -> Result<Self> {
        // Get raw content
        let raw = std::fs::read_to_string(path)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        // Deserialize content in TOML format
        toml::from_str(&raw).map_err(|error| Error::Config(format!("{}: {error}", path.display())))
    }
}
