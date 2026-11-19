use std::path::PathBuf;

use serde::Deserialize;

/// Database config.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DatabaseConfig {
    /// SQLite file, relative to the config file. Created if missing.
    pub(crate) path: PathBuf,
    /// Note that new todos go to when none is given.
    pub(crate) inbox: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("todo.db"),
            inbox: "misc".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<DatabaseConfig>(
            r#"
            path = "/var/lib/todo/todo.db"
            inbox = "todo"
            "#,
        )
        .expect("failed to deserialize string");
        assert_eq!(config.path, PathBuf::from("/var/lib/todo/todo.db"));
        assert_eq!(config.inbox, "todo");
        let config = toml::from_str::<DatabaseConfig>("").expect("empty config is valid");
        assert_eq!(config.path, PathBuf::from("todo.db"));
        assert_eq!(config.inbox, "misc");
    }
}
