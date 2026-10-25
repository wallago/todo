use serde::Deserialize;

/// Server Config.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ServerConfig {
    /// Address to listen.
    pub(crate) address: String,
    /// Port to listen.
    pub(crate) port: u16,
}

// Localhost only until the API has a token.
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".into(),
            port: 3000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<ServerConfig>(
            r#"
            address = "0.0.0.0"
            port = 3000
            "#,
        )
        .expect("failed to deserialize string");
        assert_eq!(config.address, "0.0.0.0");
        assert_eq!(config.port, 3000);
        let config = toml::from_str::<ServerConfig>("").expect("empty config is valid");
        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(
            toml::from_str::<ServerConfig>("address = [\"0.0.0.0\"]\nport = [\"3000\"]").is_err()
        );
    }
}
