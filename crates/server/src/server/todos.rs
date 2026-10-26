//! Todo routes: `/api/todos` and `/api/notes`.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::NaiveDate;
use todo_client::{Id, NewTodo, Status, Todo, TodoPatch, TodoQuery};

use super::api_error::ApiError;
use crate::app::AppState;

/// Todo routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/todos", get(list).post(add))
        .route("/api/todos/{id}", get(one).patch(update).delete(remove))
        .route("/api/todos/{id}/complete", post(complete))
        .route("/api/notes", get(notes))
}

/// `GET /api/todos`: every todo, or only those of `?note=`.
async fn list(State(state): State<AppState>, Query(query): Query<TodoQuery>) -> Json<Vec<Todo>> {
    let todos = state.list_todos(query.note.as_deref());
    Json(todos.into_iter().map(|(_, todo)| todo).collect())
}

/// `GET /api/notes`: every note's path, relative to the vault.
async fn notes(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(state.list_notes())
}

/// `GET /api/todos/{id}`: one todo, or 404.
async fn one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    let todo = state.store().get(&Id(id.clone())).cloned();
    todo.map(Json).ok_or_else(|| ApiError::not_found(&id))
}

/// `POST /api/todos`: adds a todo to the inbox note, or to `note`. Answers 201.
async fn add(
    State(state): State<AppState>,
    Json(new): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), ApiError> {
    Ok((StatusCode::CREATED, Json(state.add_todo(new)?)))
}

/// `PATCH /api/todos/{id}`: changes the fields that are sent.
async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<TodoPatch>,
) -> Result<Json<Todo>, ApiError> {
    Ok(Json(state.update_todo(&Id(id), patch)?))
}

/// `POST /api/todos/{id}/complete`: ticks it. Safe to retry.
async fn complete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    Ok(Json(state.complete_todo(&Id(id))?))
}

/// `DELETE /api/todos/{id}`: removes it and its line. Answers 204.
async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.remove_todo(&Id(id))?;
    Ok(StatusCode::NO_CONTENT)
}

/// What the routes and the MCP tools both do.
impl AppState {
    /// Every todo with its note's path, or only those of `note`.
    pub(super) fn list_todos(&self, note: Option<&str>) -> Vec<(String, Todo)> {
        self.store()
            .todos_by_note()
            .filter(|(path, _)| note.is_none_or(|note| *path == std::path::Path::new(note)))
            .map(|(path, todo)| (path.to_string_lossy().into_owned(), todo.clone()))
            .collect()
    }

    /// Every note's path, relative to the vault.
    pub(super) fn list_notes(&self) -> Vec<String> {
        self.store()
            .notes()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }

    /// Adds a todo to the inbox note, or to `new.note`.
    pub(super) fn add_todo(&self, new: NewTodo) -> Result<Todo, ApiError> {
        check(Some(&new.text), Some(&new.tags))?;
        let mut todo = Todo::new(Id(String::new()), new.text.trim(), today());
        todo.priority = new.priority;
        todo.due = new.due;
        todo.tags = new.tags;
        let mut store = self.store();
        Ok(match new.note {
            Some(note) => store.add_to(note, todo)?,
            None => store.add(todo)?,
        })
    }

    /// Changes the fields that are set in `patch`.
    pub(super) fn update_todo(&self, id: &Id, patch: TodoPatch) -> Result<Todo, ApiError> {
        check(patch.text.as_deref(), patch.tags.as_deref())?;
        let today = today();
        Ok(self.store().update(id, |todo| apply(patch, todo, today))?)
    }

    /// Ticks a todo.
    pub(super) fn complete_todo(&self, id: &Id) -> Result<Todo, ApiError> {
        let today = today();
        Ok(self.store().update(id, |todo| todo.complete(today))?)
    }

    /// Removes a todo and its line.
    pub(super) fn remove_todo(&self, id: &Id) -> Result<(), ApiError> {
        self.store().remove(id)?;
        Ok(())
    }
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
    async fn add_can_target_another_note() {
        let (dir, app) = test_app(NOTE);
        let new = json!({"text": "ship it", "note": "projects/dev.md"});
        let (status, body) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CREATED);
        let note = std::fs::read_to_string(dir.path().join("projects/dev.md")).expect("note");
        assert_eq!(
            note,
            format!(
                "- [ ] ship it ➕ {} ^{}\n",
                today(),
                body["id"].as_str().unwrap_or_default()
            )
        );

        let (_, body) = send(&app, Method::GET, "/api/notes", None).await;
        assert_eq!(body, json!(["projects/dev.md", "todo.md"]));
        let (_, body) = send(&app, Method::GET, "/api/todos?note=projects/dev.md", None).await;
        assert_eq!(body.as_array().map(Vec::len), Some(1), "{body}");
        assert_eq!(body[0]["text"], "ship it");

        let new = json!({"text": "x", "note": "../outside.md"});
        let (status, body) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("../outside.md"),
            "{body}"
        );
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

    #[tokio::test]
    async fn client_round_trip() -> todo_client::Result<()> {
        let (_dir, app) = test_app(NOTE);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let client = todo_client::Client::new(&format!("http://{addr}/"));

        assert_eq!(client.list().await?.len(), 2);
        let new = NewTodo {
            text: "buy milk".into(),
            note: Some("errands/shop.md".into()),
            ..NewTodo::default()
        };
        let todo = client.add(&new).await?;
        assert_eq!(client.notes().await?, ["errands/shop.md", "todo.md"]);
        assert_eq!(
            client.list_note("errands/shop.md").await?,
            std::slice::from_ref(&todo)
        );
        assert_eq!(client.get(&todo.id).await?, todo);
        let patch = TodoPatch {
            due: Some(None),
            ..TodoPatch::default()
        };
        assert_eq!(client.update(&Id("aaaaaa".into()), &patch).await?.due, None);
        assert_eq!(client.complete(&todo.id).await?.status, Status::Done);
        client.remove(&todo.id).await?;
        assert!(client.get(&todo.id).await.is_err_and(|e| e.is_not_found()));

        // The server's message comes through.
        let empty = NewTodo::default();
        match client.add(&empty).await {
            Err(todo_client::Error::Api { status, message }) => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert_eq!(message, "text is empty");
            }
            other => panic!("expected a 400, got {other:?}"),
        }
        Ok(())
    }
}
