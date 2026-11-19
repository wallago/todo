//! Todo routes: `/api/todos`, `/api/notes` and `/api/sync`.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::NaiveDate;
use todo_client::{Changes, Id, NewTodo, SyncQuery, Todo, TodoPatch, TodoQuery};

use super::api_error::ApiError;
use crate::app::AppState;

/// Todo routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/todos", get(list).post(add))
        .route("/api/todos/{id}", get(one).patch(update).delete(remove))
        .route("/api/todos/{id}/complete", post(complete))
        .route("/api/notes", get(notes))
        .route("/api/sync", get(sync))
}

/// `GET /api/todos`: every todo, or only those of `?note=`.
async fn list(
    State(state): State<AppState>,
    Query(query): Query<TodoQuery>,
) -> Result<Json<Vec<Todo>>, ApiError> {
    let todos = state.list_todos(query.note.as_deref())?;
    Ok(Json(todos.into_iter().map(|(_, todo)| todo).collect()))
}

/// `GET /api/notes`: every note's name, like `projects/dev`.
async fn notes(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(state.list_notes()?))
}

/// `GET /api/sync?since=N`: every note and todo changed after cursor `N`
/// (0: everything), deleted ones included, and the next cursor.
async fn sync(
    State(state): State<AppState>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Changes>, ApiError> {
    Ok(Json(state.store().changes(query.since)?))
}

/// `GET /api/todos/{id}`: one todo, or 404.
async fn one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    let todo = state.store().get(&Id(id.clone()))?;
    todo.map(Json).ok_or_else(|| ApiError::not_found(&id))
}

/// `POST /api/todos`: adds a todo to `note`. Answers
/// 201, or 200 with the saved todo if `id` was already added.
async fn add(
    State(state): State<AppState>,
    Json(new): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), ApiError> {
    let (todo, created) = state.add_todo(&new)?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(todo)))
}

/// `PATCH /api/todos/{id}`: changes the fields that are sent.
async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<TodoPatch>,
) -> Result<Json<Todo>, ApiError> {
    Ok(Json(state.update_todo(&Id(id), patch)?))
}

/// `POST /api/todos/{id}/complete`: ticks it. Safe to retry. A recurring
/// todo also gets its next one added.
async fn complete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, ApiError> {
    Ok(Json(state.complete_todo(&Id(id))?))
}

/// `DELETE /api/todos/{id}`: removes it. Answers 204, also if it was
/// already removed, so an offline client can retry.
async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.remove_todo(&Id(id))?;
    Ok(StatusCode::NO_CONTENT)
}

/// What the routes and the MCP tools both do.
impl AppState {
    /// Every todo with its note's name, or only those of `note`.
    pub(super) fn list_todos(&self, note: Option<&str>) -> Result<Vec<(String, Todo)>, ApiError> {
        Ok(self.store().todos_by_note(note)?)
    }

    /// Every note's name.
    pub(super) fn list_notes(&self) -> Result<Vec<String>, ApiError> {
        Ok(self.store().notes()?)
    }

    /// Adds a todo to `new.note`. With an `id`
    /// already saved, returns that todo instead; `false` then.
    pub(super) fn add_todo(&self, new: &NewTodo) -> Result<(Todo, bool), ApiError> {
        check(Some(&new.text), Some(&new.tags))?;
        if let Some(id) = &new.id {
            if uuid::Uuid::parse_str(&id.0).is_err() {
                return Err(ApiError::bad_request(format!(
                    "id `{}` is not a UUID",
                    id.0
                )));
            }
            if let Some(saved) = self.store().get(id)? {
                return Ok((saved, false));
            }
        }
        let todo = new.to_todo(today());
        Ok((self.store().add_to(&new.note, todo)?, true))
    }

    /// Changes the fields that are set in `patch`.
    pub(super) fn update_todo(&self, id: &Id, patch: TodoPatch) -> Result<Todo, ApiError> {
        check(patch.text.as_deref(), patch.tags.as_deref())?;
        let today = today();
        Ok(self.store().update(id, |todo| patch.apply(todo, today))?)
    }

    /// Ticks a todo.
    pub(super) fn complete_todo(&self, id: &Id) -> Result<Todo, ApiError> {
        let today = today();
        Ok(self.store().update(id, |todo| todo.complete(today))?)
    }

    /// Removes a todo.
    pub(super) fn remove_todo(&self, id: &Id) -> Result<(), ApiError> {
        let mut store = self.store();
        match store.remove(id) {
            Err(todo_store::Error::NotFound(_)) if store.is_deleted(id)? => Ok(()),
            other => other.map(drop).map_err(ApiError::from),
        }
    }
}

/// Today, in the server's time zone.
pub(super) fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// Refuses empty or multi-line text, and tags with spaces or a `#`.
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

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, header};
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use todo_client::Status;
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
        let (_state, app) = test_app(NOTE);
        let (status, body) = send(&app, Method::GET, "/api/todos", None).await;
        assert_eq!(status, StatusCode::OK);
        let todos: Vec<Todo> = serde_json::from_value(body).expect("todos");
        let texts: Vec<&str> = todos.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[tokio::test]
    async fn one_returns_the_todo_or_404() {
        let (_state, app) = test_app(NOTE);
        let (status, body) = send(&app, Method::GET, "/api/todos/bbbbbb", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "b");

        let (status, body) = send(&app, Method::GET, "/api/todos/zzzzzz", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "no todo with id `zzzzzz`");
    }

    #[tokio::test]
    async fn add_writes_to_its_note() {
        let (state, app) = test_app(NOTE);
        let new =
            json!({"note": "todo", "text": " buy milk ", "priority": "high", "tags": ["errand"]});
        let (status, body) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CREATED);
        let todo: Todo = serde_json::from_value(body).expect("todo");
        assert_eq!(todo.text, "buy milk");
        assert_eq!(todo.priority, todo_client::Priority::High);
        assert_eq!(todo.created, Some(today()));
        assert_eq!(todo.tags, ["errand"]);
        let saved = state.list_todos(Some("todo")).expect("todos");
        assert_eq!(saved.last().map(|(_, t)| t), Some(&todo));
    }

    #[tokio::test]
    async fn add_can_target_another_note() {
        let (_state, app) = test_app(NOTE);
        let new = json!({"text": "ship it", "note": "projects/dev"});
        let (status, _) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CREATED);

        let (_, body) = send(&app, Method::GET, "/api/notes", None).await;
        assert_eq!(body, json!(["projects/dev", "todo"]));
        let (_, body) = send(&app, Method::GET, "/api/todos?note=projects/dev", None).await;
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
    async fn add_needs_a_note() {
        let (_state, app) = test_app(NOTE);
        let (status, _) = send(&app, Method::POST, "/api/todos", Some(json!({"text": "x"}))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let new = json!({"text": "x", "note": ""});
        let (status, _) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_refuses_text_that_breaks_the_line() {
        let (_state, app) = test_app(NOTE);
        for new in [
            json!({"note": "todo", "text": "  "}),
            json!({"note": "todo", "text": "two\nlines"}),
            json!({"note": "todo", "text": "ok", "tags": ["two words"]}),
        ] {
            let (status, _) = send(&app, Method::POST, "/api/todos", Some(new)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn patch_changes_only_what_is_sent() {
        let (state, app) = test_app(NOTE);
        let patch = json!({"note": "todo", "text": "a, renamed", "due": null});
        let (status, body) = send(&app, Method::PATCH, "/api/todos/aaaaaa", Some(patch)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["text"], "a, renamed");
        assert_eq!(body.get("due"), None);
        let saved = state.store().get(&Id("aaaaaa".into())).expect("get");
        assert_eq!(
            saved.map(|t| (t.text, t.due)),
            Some(("a, renamed".into(), None))
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
    async fn pin_and_recur_over_the_api() {
        let (_state, app) = test_app(NOTE);
        let new = json!({"note": "todo", "text": "gym", "pinned": true, "recur": {"every": 2, "unit": "day"}});
        let (status, body) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["pinned"], true);
        assert_eq!(body["recur"], json!({"every": 2, "unit": "day"}));
        let id = body["id"].as_str().unwrap_or_default().to_owned();

        let (_, done) = send(
            &app,
            Method::POST,
            &format!("/api/todos/{id}/complete"),
            None,
        )
        .await;
        assert_eq!(done["status"], "done");
        let (_, todos) = send(&app, Method::GET, "/api/todos", None).await;
        let gyms: Vec<&Value> = todos
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| t["text"] == "gym")
            .collect();
        assert_eq!(gyms.len(), 2, "{todos}");
        let next = gyms[1];
        assert_eq!(next["status"], "todo");
        assert_eq!(
            next["due"],
            json!((today() + chrono::Days::new(2)).to_string())
        );

        let patch = json!({"pinned": false, "recur": null});
        let uri = format!("/api/todos/{}", next["id"].as_str().unwrap_or_default());
        let (_, body) = send(&app, Method::PATCH, &uri, Some(patch)).await;
        assert_eq!((body.get("pinned"), body.get("recur")), (None, None));
    }

    #[tokio::test]
    async fn complete_is_safe_to_retry() {
        let (_state, app) = test_app(NOTE);
        let (status, first) = send(&app, Method::POST, "/api/todos/aaaaaa/complete", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["status"], "done");
        let (_, again) = send(&app, Method::POST, "/api/todos/aaaaaa/complete", None).await;
        assert_eq!(again, first);
    }

    #[tokio::test]
    async fn delete_removes_the_todo() {
        let (_state, app) = test_app(NOTE);
        let (status, _) = send(&app, Method::DELETE, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = send(&app, Method::GET, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // Again: already gone is fine, an unknown id is not.
        let (status, _) = send(&app, Method::DELETE, "/api/todos/aaaaaa", None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = send(&app, Method::DELETE, "/api/todos/zzzzzz", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn add_with_a_client_id_is_safe_to_retry() {
        let (_state, app) = test_app(NOTE);
        let id = "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
        let new = json!({"id": id, "text": "offline", "note": "phone"});
        let (status, first) = send(&app, Method::POST, "/api/todos", Some(new.clone())).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(first["id"], id);
        let (status, again) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(again, first);
        let (_, todos) = send(&app, Method::GET, "/api/todos?note=phone", None).await;
        assert_eq!(todos.as_array().map(Vec::len), Some(1));

        let bad = json!({"id": "not-a-uuid", "text": "x", "note": "todo"});
        let (status, _) = send(&app, Method::POST, "/api/todos", Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // A deleted id can't come back.
        send(&app, Method::DELETE, &format!("/api/todos/{id}"), None).await;
        let new = json!({"id": id, "text": "offline", "note": "todo"});
        let (status, _) = send(&app, Method::POST, "/api/todos", Some(new)).await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn sync_returns_changes_after_the_cursor() {
        let (_state, app) = test_app(NOTE);
        let (status, all) = send(&app, Method::GET, "/api/sync", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(all["notes"], json!([{"name": "todo"}]));
        assert_eq!(all["todos"].as_array().map(Vec::len), Some(2));
        let cursor = all["cursor"].as_u64().unwrap_or_default();

        send(&app, Method::POST, "/api/todos/aaaaaa/complete", None).await;
        send(&app, Method::DELETE, "/api/todos/bbbbbb", None).await;
        let uri = format!("/api/sync?since={cursor}");
        let (_, next) = send(&app, Method::GET, &uri, None).await;
        assert_eq!(next["notes"], json!([]));
        let todos = next["todos"].as_array().cloned().unwrap_or_default();
        let seen: Vec<(Value, Value, Value)> = todos
            .iter()
            .map(|t| (t["id"].clone(), t["status"].clone(), t["deleted"].clone()))
            .collect();
        assert_eq!(
            seen,
            [
                (json!("aaaaaa"), json!("done"), Value::Null),
                (json!("bbbbbb"), json!("done"), json!(true)),
            ]
        );
        assert_eq!(todos[0]["note"], "todo");
    }

    #[tokio::test]
    async fn client_round_trip() -> todo_client::Result<()> {
        let (_state, app) = test_app(NOTE);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let client = todo_client::Client::new(&format!("http://{addr}/"));

        assert_eq!(client.list().await?.len(), 2);
        let new = NewTodo {
            text: "buy milk".into(),
            note: "errands/shop".into(),
            ..NewTodo::default()
        };
        let todo = client.add(&new).await?;
        assert_eq!(client.notes().await?, ["errands/shop", "todo"]);
        assert_eq!(
            client.list_note("errands/shop").await?,
            std::slice::from_ref(&todo)
        );
        assert_eq!(client.get(&todo.id).await?, todo);
        let patch = TodoPatch {
            due: Some(None),
            ..TodoPatch::default()
        };
        assert_eq!(client.update(&Id("aaaaaa".into()), &patch).await?.due, None);
        assert_eq!(client.complete(&todo.id).await?.status, Status::Done);
        let synced = client.sync(0).await?;
        assert!(synced.notes.iter().any(|n| n.name == "errands/shop"));
        client.remove(&todo.id).await?;
        assert!(client.get(&todo.id).await.is_err_and(|e| e.is_not_found()));
        let after = client.sync(synced.cursor).await?;
        assert_eq!(
            after
                .todos
                .iter()
                .map(|c| (&c.todo.id, c.deleted))
                .collect::<Vec<_>>(),
            [(&todo.id, true)]
        );

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
