//! Todo routes: `/api/todos`.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::NaiveDate;
use todo_client::{Id, NewTodo, Status, Todo, TodoPatch};

use super::api_error::ApiError;
use crate::app::AppState;

/// Todo routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/todos", get(list).post(add))
        .route("/api/todos/{id}", get(one).patch(update).delete(remove))
        .route("/api/todos/{id}/complete", post(complete))
}

/// `GET /api/todos`: every todo.
async fn list(State(state): State<AppState>) -> Json<Vec<Todo>> {
    Json(state.store().todos().cloned().collect())
}

/// `GET /api/todos/{id}`: one todo, or 404.
async fn one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    let todo = state.store().get(&Id(id.clone())).cloned();
    todo.map(Json).ok_or_else(|| ApiError::not_found(&id))
}

/// `POST /api/todos`: adds a todo to the inbox note. Answers 201.
async fn add(
    State(state): State<AppState>,
    Json(new): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), ApiError> {
    check(Some(&new.text), Some(&new.tags))?;
    let mut todo = Todo::new(Id(String::new()), new.text.trim(), today());
    todo.priority = new.priority;
    todo.due = new.due;
    todo.tags = new.tags;
    let todo = state.store().add(todo)?;
    Ok((StatusCode::CREATED, Json(todo)))
}

/// `PATCH /api/todos/{id}`: changes the fields that are sent.
async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<TodoPatch>,
) -> Result<Json<Todo>, ApiError> {
    check(patch.text.as_deref(), patch.tags.as_deref())?;
    let today = today();
    let todo = state
        .store()
        .update(&Id(id), |todo| apply(patch, todo, today))?;
    Ok(Json(todo))
}

/// `POST /api/todos/{id}/complete`: ticks it. Safe to retry.
async fn complete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    let today = today();
    let todo = state.store().update(&Id(id), |todo| todo.complete(today))?;
    Ok(Json(todo))
}

/// `DELETE /api/todos/{id}`: removes it and its line. Answers 204.
async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.store().remove(&Id(id))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Today, in the server's time zone.
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// Refuses text or tags that would break the task's line in the note.
fn check(text: Option<&str>, tags: Option<&[String]>) -> Result<(), ApiError> {
    if let Some(text) = text {
        if text.trim().is_empty() {
            return Err(ApiError::bad_request("text is empty"));
        }
        if text.contains(['\n', '\r']) {
            return Err(ApiError::bad_request("text has a line break"));
        }
    }
    let bad_tag =
        |tag: &&String| tag.is_empty() || tag.starts_with('#') || tag.contains(char::is_whitespace);
    if let Some(tag) = tags.unwrap_or_default().iter().find(bad_tag) {
        return Err(ApiError::bad_request(format!(
            "bad tag `{tag}`: no spaces, no leading #"
        )));
    }
    Ok(())
}

/// Applies a patch. Status goes through `complete`/`reopen` so the done
/// date stays right.
fn apply(patch: TodoPatch, todo: &mut Todo, today: NaiveDate) {
    if let Some(text) = patch.text {
        text.trim().clone_into(&mut todo.text);
    }
    if let Some(priority) = patch.priority {
        todo.priority = priority;
    }
    if let Some(due) = patch.due {
        todo.due = due;
    }
    if let Some(tags) = patch.tags {
        todo.tags = tags;
    }
    match patch.status {
        Some(Status::Done) => todo.complete(today),
        Some(Status::Todo) => todo.reopen(),
        Some(status) => {
            todo.status = status;
            todo.done = None;
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, header};
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_app;

    /// The note every test starts from.
    const NOTE: &str = "# Todo\n- [ ] a 📅 2026-10-30 ^aaaaaa\n- [x] b ✅ 2026-10-20 ^bbbbbb\n";

    /// Sends a request; returns the status and the JSON body (`null` if empty).
    async fn send(
        app: &Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("valid request");
        let response = app.clone().oneshot(request).await.expect("infallible");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn list_returns_every_todo() {
        let (_dir, app) = test_app(NOTE);
        let (status, body) = send(&app, Method::GET, "/api/todos", None).await;
        assert_eq!(status, StatusCode::OK);
        let todos: Vec<Todo> = serde_json::from_value(body).expect("todos");
        let texts: Vec<&str> = todos.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[tokio::test]
    async fn one_returns_the_todo_or_404() {
        let (_dir, app) = test_app(NOTE);
        let (status, body) = send(&app, Method::GET, "/api/todos/bbbbbb", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "b");

        let (status, body) = send(&app, Method::GET, "/api/todos/zzzzzz", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "no todo with id `zzzzzz`");
    }

    #[tokio::test]
    async fn add_writes_to_the_inbox() {
        let (dir, app) = test_app(NOTE);
        let new = json!({"text": " buy milk ", "priority": "high", "tags": ["errand"]});
        let (status, body) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CREATED);
        let todo: Todo = serde_json::from_value(body).expect("todo");
        assert_eq!(todo.text, "buy milk");
        let note = std::fs::read_to_string(dir.path().join("todo.md")).expect("note");
        let line = format!("- [ ] buy milk ⏫ ➕ {} #errand ^{}", today(), todo.id.0);
        assert!(note.contains(&line), "{note}");
    }

    #[tokio::test]
    async fn add_refuses_text_that_breaks_the_line() {
        let (_dir, app) = test_app(NOTE);
        for new in [
            json!({"text": "  "}),
            json!({"text": "two\nlines"}),
            json!({"text": "ok", "tags": ["two words"]}),
        ] {
            let (status, _) = send(&app, Method::POST, "/api/todos", Some(new)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn patch_changes_only_what_is_sent() {
        let (dir, app) = test_app(NOTE);
        let patch = json!({"text": "a, renamed", "due": null});
        let (status, body) = send(&app, Method::PATCH, "/api/todos/aaaaaa", Some(patch)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "a, renamed");
        assert_eq!(body.get("due"), None);
        let note = std::fs::read_to_string(dir.path().join("todo.md")).expect("note");
        assert_eq!(
            note,
            "# Todo\n- [ ] a, renamed ^aaaaaa\n- [x] b ✅ 2026-10-20 ^bbbbbb\n"
        );

        // Reopening clears the done date.
        let (_, body) = send(
            &app,
            Method::PATCH,
            "/api/todos/bbbbbb",
            Some(json!({"status": "todo"})),
        )
        .await;
        assert_eq!(body["status"], "todo");
        assert_eq!(body.get("done"), None);
    }

    #[tokio::test]
    async fn complete_is_safe_to_retry() {
        let (_dir, app) = test_app(NOTE);
        let (status, first) = send(&app, Method::POST, "/api/todos/aaaaaa/complete", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["status"], "done");
        let (_, again) = send(&app, Method::POST, "/api/todos/aaaaaa/complete", None).await;
        assert_eq!(again, first);
    }

    #[tokio::test]
    async fn delete_removes_the_todo() {
        let (_dir, app) = test_app(NOTE);
        let (status, _) = send(&app, Method::DELETE, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = send(&app, Method::GET, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&app, Method::DELETE, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
