//! Server core.

use axum::{Router, http::StatusCode, routing::get, serve};

use crate::{app::AppState, prelude::*};

/// `/api/todos`.
mod todos;

/// Liveness probe: answers as long as the server is up.
async fn health() -> StatusCode {
    StatusCode::OK
}

/// Global server routes.
fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(todos::routes())
        .with_state(state)
}

/// Start web services. Returns on Ctrl-C or SIGTERM (systemd stop).
pub(super) async fn start(address: &str, port: u16, state: AppState) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(format!("{address}:{port}")).await?;
    tracing::info!("Server running on {}", listener.local_addr()?);
    serve(listener, routes(state))
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

/// A router over a temp vault holding one `todo.md` note.
#[cfg(test)]
fn test_app(note: &str) -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("todo.md"), note).expect("write note");
    let store = todo_store::Store::open(dir.path(), "todo.md").expect("open vault");
    (dir, routes(AppState::new(store)))
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
