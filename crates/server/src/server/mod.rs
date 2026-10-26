//! Server core.

use axum::{Router, http::StatusCode, middleware, routing::get, serve};

use crate::{
    app::AppState,
    config::{mcp::McpConfig, server::ServerConfig},
    prelude::*,
};

/// Errors answered as JSON.
mod api_error;
/// Bearer token.
mod auth;
/// `/mcp`.
mod mcp;
/// `/api/todos`.
mod todos;

/// Liveness probe: answers as long as the server is up.
async fn health() -> StatusCode {
    StatusCode::OK
}

/// Global server routes. With a `token`, `/api` and `/mcp` need it.
fn routes(state: AppState, mcp_config: &McpConfig, token: Option<auth::Token>) -> Router {
    let protected = Router::new()
        .merge(todos::routes())
        .nest_service("/mcp", mcp::service(state.clone(), mcp_config));
    let protected = match token {
        Some(token) => protected.layer(middleware::from_fn_with_state(token, auth::require)),
        None => protected,
    };
    Router::new()
        .route("/health", get(health))
        .merge(protected)
        .with_state(state)
}

/// Start web services. Returns on Ctrl-C or SIGTERM (systemd stop).
pub(super) async fn start(
    config: &ServerConfig,
    state: AppState,
    mcp_config: &McpConfig,
) -> Result<()> {
    let token = config
        .token_file
        .as_deref()
        .map(auth::Token::read)
        .transpose()?;
    if token.is_none() {
        tracing::warn!("no `token_file`: /api and /mcp are open to anyone who can reach them");
    }
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.address, config.port)).await?;
    tracing::info!("Server running on {}", listener.local_addr()?);
    serve(listener, routes(state, mcp_config, token))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Resolves on the first Ctrl-C or SIGTERM.
async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
}

/// A router over a temp vault holding one `todo.md` note, without a token.
#[cfg(test)]
fn test_app(note: &str) -> (tempfile::TempDir, Router) {
    test_router(note, None)
}

/// Same as [`test_app`], with this token.
#[cfg(test)]
fn test_router(note: &str, token: Option<auth::Token>) -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("todo.md"), note).expect("write note");
    let store = todo_store::Store::open(dir.path(), "todo.md").expect("open vault");
    (
        dir,
        routes(AppState::new(store), &McpConfig::default(), token),
    )
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_is_ok() {
        let (_dir, app) = test_app("");
        let request = Request::get("/health")
            .body(Body::empty())
            .expect("valid request");
        let response = app.oneshot(request).await.expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
    }
}
