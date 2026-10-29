//! Config file setup.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::prelude::*;

/// Configuration loaded from `config.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    /// The todo server to talk to.
    pub(crate) server: ServerConfig,
}

/// `[server]`: where the todo server is.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ServerConfig {
    /// Base URL, like `http://127.0.0.1:3000`.
    pub(crate) url: String,
    /// File holding the server's bearer token, relative to the config file.
    pub(crate) token_file: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:3000".to_owned(),
            token_file: None,
        }
    }
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
        // Relative paths are relative to the config file, not the cwd.
        if let Some(dir) = path.parent() {
            config.server.token_file = config.server.token_file.map(|file| dir.join(file));
        }
        Ok(config)
    }

    /// A client for the configured server, with its token if there is one.
    ///
    /// # Errors
    ///
    /// Returns an error if the token file can't be read or is empty.
    pub(crate) fn client(&self) -> Result<todo_client::Client> {
        let client = todo_client::Client::new(&self.server.url);
        let Some(path) = &self.server.token_file else {
            return Ok(client);
        };
        let raw = std::fs::read_to_string(path)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        let token = raw.trim();
        if token.is_empty() {
            return Err(Error::Config(format!("{}: token is empty", path.display())));
        }
        Ok(client.with_token(token))
    }
}
