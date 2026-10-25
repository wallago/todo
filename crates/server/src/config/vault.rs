use std::path::PathBuf;

use serde::Deserialize;

/// Vault Config.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VaultConfig {
    /// Vault root path.
    pub(crate) root: PathBuf,
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
        assert!(toml::from_str::<VaultConfig>("root = [\"../../../../vault/\"]").is_err());
    }
}
