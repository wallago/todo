use std::path::PathBuf;

use serde::Deserialize;

/// Server Config.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ServerConfig {
    /// Address to listen.
    pub(crate) address: String,
    /// Port to listen.
    pub(crate) port: u16,
    /// File holding the bearer token for `/api` and `/mcp`. Unset: no auth.
    #[serde(alias = "tokenFile")]
    pub(crate) token_file: Option<PathBuf>,
    /// Public HTTPS URL, e.g. `https://todo.example.com`. Set with
    /// `token_file`, it turns on OAuth so claude.ai can add `/mcp` as a
    /// custom connector.
    #[serde(alias = "publicUrl")]
    pub(crate) public_url: Option<String>,
}

// Localhost by default. Set a `token_file` before listening wider.
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".into(),
            port: 3000,
            token_file: None,
            public_url: None,
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
            token_file = "/run/agenix/todo-token"
            publicUrl = "https://todo.example.com"
            "#,
        )
        .expect("failed to deserialize string");
        assert_eq!(config.address, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(
            config.token_file,
            Some(PathBuf::from("/run/agenix/todo-token"))
        );
        assert_eq!(
            config.public_url.as_deref(),
            Some("https://todo.example.com")
        );
        let config = toml::from_str::<ServerConfig>("").expect("empty config is valid");
        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.token_file, None);
        assert!(
            toml::from_str::<ServerConfig>("address = [\"0.0.0.0\"]\nport = [\"3000\"]").is_err()
        );
    }
}
