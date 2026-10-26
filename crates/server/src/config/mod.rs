//! Config file setup.

use std::path::Path;

use serde::Deserialize;

use crate::prelude::*;

/// MCP settings.
pub(crate) mod mcp;
/// Server settings.
pub(crate) mod server;
/// Vault settings.
pub(crate) mod vault;

/// Configuration loaded from `config.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Server config.
    #[serde(default)]
    pub(crate) server: server::ServerConfig,
    /// Vault config.
    pub(crate) vault: vault::VaultConfig,
    /// MCP config.
    #[serde(default)]
    pub(crate) mcp: mcp::McpConfig,
}

impl Config {
    /// Loads the config: explicit `--config` path, else
    /// `$XDG_CONFIG_HOME/env!("CARGO_PKG_NAME")/config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if no config file is found, or it cannot be read or parsed.
    pub(crate) fn load(cli_path: Option<&Path>) -> Result<Self> {
        if let Some(path) = cli_path {
            return Self::from_file(path);
        }
        match dirs::config_dir()
            .map(|dir| dir.join(format!("{}/config.toml", env!("CARGO_PKG_NAME"))))
        {
            Some(path) if path.exists() => Self::from_file(&path),
            Some(path) => Err(Error::Config(format!(
                "no config file: pass --config or create {}",
                path.display()
            ))),
            None => Err(Error::Config("no config file: pass --config".to_owned())),
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
        // Relative paths are relative to the config file, not the cwd.
        if let Some(dir) = path.parent() {
            config.vault.root = dir.join(&config.vault.root);
            config.server.token_file = config.server.token_file.map(|file| dir.join(file));
        }
        Ok(config)
    }
}
