//! Command-line arguments parser.

use clap::Parser;

/// Argument parser powered by [`clap`].
#[derive(Clone, Debug, Default, Parser)]
#[clap(
    version,
    author = clap::crate_authors!("\n"),
    about,
    rename_all_env = "screaming-snake",
    help_template = "\
{before-help}server {version}
{author-with-newline}{about-with-newline}
{usage-heading}
  {usage}

{all-args}{after-help}
",
)]
pub struct Args {
    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Path to the application config file (TOML).
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<std::path::PathBuf>,

    /// What to do. Serves the API when left out.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcommands.
#[derive(Clone, Debug, clap::Subcommand)]
pub enum Command {
    /// Copies every todo of an Obsidian vault into the database, then exits.
    /// Never changes the vault. Safe to run twice: known ids are skipped.
    Import {
        /// Vault folder.
        vault: std::path::PathBuf,
        /// Database file. Defaults to `[database] path` from the config.
        #[arg(long, value_name = "PATH")]
        db: Option<std::path::PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    #[test]
    fn test_args() {
        Args::command().debug_assert();
    }
}
