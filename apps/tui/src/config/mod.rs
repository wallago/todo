//! Config file setup.

use std::path::Path;

use serde::Deserialize;

use crate::prelude::*;

/// Movement keys.
pub(crate) mod keys;
/// Where the todo server is.
pub(crate) mod server;

pub(crate) use keys::Keys;
pub(crate) use server::ServerConfig;

/// Configuration loaded from `config.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    /// The todo server to talk to.
    pub(crate) server: ServerConfig,
    /// Movement keys, for layouts other than QWERTY.
    pub(crate) keys: Keys,
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
            _ => Ok(Self::default()),
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
        let mut config: Self = toml::from_str(&raw)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        config
            .keys
            .check()
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        // Relative paths are relative to the config file, not the cwd.
        if let Some(dir) = path.parent() {
            config.server.token_file = config.server.token_file.map(|file| dir.join(file));
        }
        Ok(config)
    }
}
