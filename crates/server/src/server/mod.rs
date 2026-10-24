//! Server core.

use axum::{Router, http::StatusCode, routing::get, serve};

use crate::prelude::*;


/// Liveness probe: answers as long as the server is up.
async fn health() -> StatusCode {
    StatusCode::OK
}

/// Global server routes.
fn routes() -> Router {
    Router::new().route("/health", get(health))
}

/// Start web services.
pub(super) async fn start(address: &str, port: u16) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(format!("{address}:{port}")).await?;
    tracing::info!("Server running");
    let routes = routes()
    ;
    Ok(serve(listener, routes).await?)
}
