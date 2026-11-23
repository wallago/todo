use serde::Deserialize;

/// MCP config.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct McpConfig {
    /// `Host` headers accepted on `/mcp`; anything else gets a 403.
    #[serde(alias = "allowedHosts")]
    pub(crate) allowed_hosts: Vec<String>,
}

// Loopback only until a public hostname is listed.
impl Default for McpConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: ["localhost", "127.0.0.1", "::1"].map(String::from).to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<McpConfig>(r#"allowed_hosts = ["todo.example.com"]"#)
            .expect("failed to deserialize string");
        assert_eq!(config.allowed_hosts, ["todo.example.com"]);
        let config = toml::from_str::<McpConfig>("").expect("empty config is valid");
        assert_eq!(config.allowed_hosts, ["localhost", "127.0.0.1", "::1"]);
    }
}
