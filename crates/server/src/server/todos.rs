//! Todo routes: `/api/todos`.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use todo_core::{Id, Todo};

use crate::app::AppState;

/// Todo routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/todos", get(list))
        .route("/api/todos/{id}", get(one))
}

/// `GET /api/todos`: every todo.
async fn list(State(state): State<AppState>) -> Json<Vec<Todo>> {
    Json(state.store().todos().cloned().collect())
}

/// `GET /api/todos/{id}`: one todo, or 404.
async fn one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, StatusCode> {
    state
        .store()
        .get(&Id(id))
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_app;

    /// Sends `GET uri`; returns the status and the body.
    async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
        let (_dir, app) = test_app("# Todo\n- [ ] a ^aaaaaa\n- [x] b ^bbbbbb\n");
        let request = Request::get(uri)
            .body(Body::empty())
            .expect("valid request");
        let response = app.oneshot(request).await.expect("infallible");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, body.to_vec())
    }

    #[tokio::test]
    async fn list_returns_every_todo() {
        let (status, body) = get("/api/todos").await;
        assert_eq!(status, StatusCode::OK);
        let todos: Vec<Todo> = serde_json::from_slice(&body).expect("json");
        let texts: Vec<&str> = todos.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[tokio::test]
    async fn one_returns_the_todo_or_404() {
        let (status, body) = get("/api/todos/bbbbbb").await;
        assert_eq!(status, StatusCode::OK);
        let todo: Todo = serde_json::from_slice(&body).expect("json");
        assert_eq!(todo.text, "b");

        let (status, _) = get("/api/todos/zzzzzz").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
