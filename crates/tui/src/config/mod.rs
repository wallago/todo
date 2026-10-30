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
    /// Movement keys, for layouts other than QWERTY.
    pub(crate) keys: Keys,
}

/// `[keys]`: letters that move, on top of the arrow keys.
/// In the note finder, hold Alt (or Ctrl) with them, since plain letters type.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Keys {
    /// Moves up a row.
    pub(crate) up: char,
    /// Moves down a row.
    pub(crate) down: char,
    /// Goes to the sidebar.
    pub(crate) left: char,
    /// Goes to the page.
    pub(crate) right: char,
}

impl Keys {
    /// Keys the app already uses, which a movement key can't take.
    const TAKEN: [char; 3] = ['q', 'r', '/'];

    /// Refuses a key used twice, or one the app already uses.
    ///
    /// # Errors
    ///
    /// Returns an error naming the clashing key.
    fn check(self) -> Result<()> {
        let all = [self.up, self.down, self.left, self.right];
        for (i, key) in all.iter().enumerate() {
            if Self::TAKEN.contains(key) {
                return Err(Error::Config(format!(
                    "[keys]: `{key}` is already used by the app"
                )));
            }
            if all[..i].contains(key) {
                return Err(Error::Config(format!("[keys]: `{key}` is set twice")));
            }
        }
        Ok(())
    }
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            up: 'k',
            down: 'j',
            left: 'h',
            right: 'l',
        }
    }
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    /// Parses `toml` and runs the same checks as [`Config::from_file`].
    fn parse(toml: &str) -> Result<Config> {
        let config: Config =
            toml::from_str(toml).map_err(|error| Error::Config(error.to_string()))?;
        config.keys.check()?;
        Ok(config)
    }

    #[test]
    fn keys_default_to_vim() -> Result<()> {
        assert_eq!(parse("")?.keys, Keys::default());
        Ok(())
    }

    #[test]
    fn keys_can_be_partly_set() -> Result<()> {
        let keys = parse("[keys]\nup = \"i\"\ndown = \"e\"\n")?.keys;
        assert_eq!((keys.up, keys.down, keys.left), ('i', 'e', 'h'));
        Ok(())
    }

    #[rstest]
    #[case::two_letters("[keys]\nup = \"ii\"")]
    #[case::empty("[keys]\nup = \"\"")]
    #[case::unknown_key("[keys]\njump = \"x\"")]
    #[case::twice("[keys]\nup = \"e\"\ndown = \"e\"")]
    #[case::clashes_with_default("[keys]\nup = \"j\"")]
    #[case::taken_by_quit("[keys]\nup = \"q\"")]
    #[case::taken_by_finder("[keys]\nleft = \"/\"")]
    fn bad_keys_are_refused(#[case] toml: &str) {
        assert!(parse(toml).is_err(), "accepted: {toml}");
    }
}
