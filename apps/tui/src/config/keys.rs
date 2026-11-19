//! `[keys]`: movement keys.

use serde::Deserialize;

use crate::prelude::*;

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
    const TAKEN: [char; 6] = ['q', 'r', '/', 'a', 'c', 'd'];

    /// Refuses a key used twice, or one the app already uses.
    ///
    /// # Errors
    ///
    /// Returns an error naming the clashing key.
    pub(super) fn check(self) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::config::Config;

    /// Parses a config file's text and runs the same checks as
    /// [`Config::from_file`].
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
    #[case::taken_by_add("[keys]\nright = \"a\"")]
    fn bad_keys_are_refused(#[case] toml: &str) {
        assert!(parse(toml).is_err(), "accepted: {toml}");
    }
}
