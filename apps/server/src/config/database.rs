use std::path::PathBuf;

use serde::Deserialize;

/// Database config.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DatabaseConfig {
    /// SQLite file, relative to the config file. Created if missing.
    pub(crate) path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("todo.db"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<DatabaseConfig>(r#"path = "/var/lib/todo/todo.db""#)
            .expect("failed to deserialize string");
        assert_eq!(config.path, PathBuf::from("/var/lib/todo/todo.db"));
        let config = toml::from_str::<DatabaseConfig>("").expect("empty config is valid");
        assert_eq!(config.path, PathBuf::from("todo.db"));
        assert!(toml::from_str::<DatabaseConfig>(r#"inbox = "misc""#).is_err());
    }
}
