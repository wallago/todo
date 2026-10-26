//! Command-line arguments. Every flag also reads an env var, for systemd.

use std::net::SocketAddr;

use clap::{Parser, ValueEnum};

/// MCP server for the todo app: tools that call the todo server.
#[derive(Clone, Debug, Parser)]
#[clap(version, about)]
pub(crate) struct Args {
    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub(crate) verbose: u8,

    /// The todo server to call.
    #[arg(long, env = "TODO_SERVER_URL", default_value = "http://127.0.0.1:3000")]
    pub(crate) server_url: String,

    /// `stdio` when spawned by the MCP client, `http` when run as a service.
    #[arg(long, env = "TODO_MCP_TRANSPORT", value_enum, default_value_t = Transport::Stdio)]
    pub(crate) transport: Transport,

    /// Where `http` listens. The endpoint is `/mcp`.
    #[arg(long, env = "TODO_MCP_LISTEN", default_value = "127.0.0.1:3001")]
    pub(crate) listen: SocketAddr,

    /// Host names `http` answers to, e.g. `homelab` or `homelab:3001`.
    /// Guards against DNS rebinding. Loopback is always allowed.
    #[arg(long, env = "TODO_MCP_ALLOWED_HOSTS", value_delimiter = ',')]
    pub(crate) allowed_hosts: Vec<String>,
}

/// How the MCP client reaches us.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Transport {
    /// stdin/stdout.
    Stdio,
    /// Streamable HTTP.
    Http,
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
