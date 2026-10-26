//! MCP server for the todo app. Holds no data: every tool calls the todo server.

use std::process::exit;
use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService, stdio};
use todo_client::Client;
use tracing::Level;

use crate::args::{Args, Transport};
use crate::tools::Todos;

/// Command-line arguments.
mod args;
/// The MCP tools.
mod tools;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let level = match args.verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };
    // Logs go to stderr: stdout is the MCP channel in stdio mode.
    let fmt = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr);
    if std::env::var_os("JOURNAL_STREAM").is_some() {
        fmt.without_time().with_ansi(false).init();
    } else {
        fmt.init();
    }

    if let Err(e) = run(args).await {
        eprintln!("{e}");
        exit(1);
    }
}

/// Serves the tools until the client hangs up (stdio) or forever (http).
async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let todos = Todos::new(Client::new(&args.server_url));
    match args.transport {
        Transport::Stdio => {
            todos.serve(stdio()).await?.waiting().await?;
        }
        Transport::Http => {
            let mut hosts = vec![
                "localhost".to_owned(),
                "127.0.0.1".to_owned(),
                "::1".to_owned(),
            ];
            hosts.extend(args.allowed_hosts);
            let service = StreamableHttpService::new(
                move || Ok(todos.clone()),
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig::default().with_allowed_hosts(hosts),
            );
            let router = axum::Router::new().nest_service("/mcp", service);
            let listener = tokio::net::TcpListener::bind(args.listen).await?;
            tracing::info!("MCP on http://{}/mcp", args.listen);
            axum::serve(listener, router).await?;
        }
    }
    Ok(())
}
