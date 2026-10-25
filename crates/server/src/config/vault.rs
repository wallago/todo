use std::path::PathBuf;

use serde::Deserialize;

/// Vault Config.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VaultConfig {
    /// Vault root path.
    pub(crate) root: PathBuf,
    /// Note that new todos go to, relative to `root`.
    #[serde(default = "default_inbox")]
    pub(crate) inbox: PathBuf,
}

/// `todo.md` at the vault root.
fn default_inbox() -> PathBuf {
    PathBuf::from("todo.md")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<VaultConfig>(
            r#"
            root = "../../../../vault/"
            "#,
        )
        .expect("failed to deserialize string");
        assert_eq!(Ok(config.root), PathBuf::from_str("../../../../vault/"));
        assert_eq!(config.inbox, PathBuf::from("todo.md"));
        assert!(toml::from_str::<VaultConfig>("root = [\"../../../../vault/\"]").is_err());
    }
}
