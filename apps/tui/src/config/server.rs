//! `[server]`: where the todo server is.

use std::path::PathBuf;

use serde::Deserialize;

use crate::prelude::*;

/// `[server]`: where the todo server is.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ServerConfig {
    /// Base URL, like `http://127.0.0.1:3000`.
    pub(crate) url: String,
    /// File holding the server's bearer token, relative to the config file.
    #[serde(alias = "tokenFile")]
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

impl ServerConfig {
    /// A client for the configured server, with its token if there is one.
    ///
    /// # Errors
    ///
    /// Returns an error if the token file can't be read or is empty.
    pub(crate) fn client(&self) -> Result<todo_client::Client> {
        let client = todo_client::Client::new(&self.url);
        let Some(path) = &self.token_file else {
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
