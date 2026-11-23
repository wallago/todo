use std::path::PathBuf;

use serde::Deserialize;

/// Web app config.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct WebConfig {
    /// The built web app (`apps/web/dist`), served at `/`. Unset: no web app.
    pub(crate) dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserializes_toml() {
        let config = toml::from_str::<WebConfig>(r#"dir = "/srv/todo-web""#)
            .expect("failed to deserialize string");
        assert_eq!(config.dir, Some(PathBuf::from("/srv/todo-web")));
        let config = toml::from_str::<WebConfig>("").expect("empty config is valid");
        assert_eq!(config.dir, None);
    }
}
