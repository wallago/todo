//! `/api/agenda`: open todos due today or overdue, shaped for phone widgets.

use std::cmp::Reverse;

use axum::{
    Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use todo_client::{Id, Priority, Todo};

use super::todos::today;
use crate::app::AppState;

/// Agenda routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/api/agenda", get(agenda))
}

/// Query of `GET /api/agenda`.
#[derive(Debug, Default, Deserialize)]
struct AgendaQuery {
    /// `json` (default) or `text`.
    #[serde(default)]
    format: Format,
}

/// How the agenda is answered.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Format {
    /// [`Agenda`] as JSON.
    #[default]
    Json,
    /// [`Agenda::text`] alone, as plain text.
    Text,
}

/// Open todos due today or before.
#[derive(Debug, Serialize)]
struct Agenda {
    /// Today, in the server's time zone.
    date: NaiveDate,
    /// How many are overdue.
    overdue: usize,
    /// How many are due today.
    today: usize,
    /// One line per todo, ready to show as is.
    text: String,
    /// Overdue first (oldest first), then today's; higher priority first.
    todos: Vec<AgendaTodo>,
}

/// One todo of the agenda.
#[derive(Debug, Serialize)]
struct AgendaTodo {
    /// Stable id.
    id: Id,
    /// What to do.
    text: String,
    /// The note it's in, relative to the vault.
    note: String,
    /// Due date.
    due: NaiveDate,
    /// `true` if due before today.
    overdue: bool,
    /// Priority.
    priority: Priority,
}

/// `GET /api/agenda`: open todos due today or overdue. `?format=text` for
/// plain lines.
async fn agenda(State(state): State<AppState>, Query(query): Query<AgendaQuery>) -> Response {
    let agenda = Agenda::build(state.list_todos(None), today());
    match query.format {
        Format::Json => Json(agenda).into_response(),
        Format::Text => agenda.text.into_response(),
    }
}

impl Agenda {
    /// Keeps the open todos due on or before `today`.
    fn build(todos: Vec<(String, Todo)>, today: NaiveDate) -> Self {
        let mut todos: Vec<AgendaTodo> = todos
            .into_iter()
            .filter(|(_, todo)| !todo.status.is_closed())
            .filter_map(|(note, todo)| {
                let due = todo.due.filter(|due| *due <= today)?;
                Some(AgendaTodo {
                    overdue: todo.is_overdue(today),
                    id: todo.id,
                    text: todo.text,
                    note,
                    due,
                    priority: todo.priority,
                })
            })
            .collect();
        todos.sort_by_key(|todo| (todo.due, Reverse(todo.priority)));
        let overdue = todos.iter().filter(|todo| todo.overdue).count();
        let text = if todos.is_empty() {
            "Nothing due".to_owned()
        } else {
            todos
                .iter()
                .map(|todo| todo.line(today))
                .collect::<Vec<_>>()
                .join("\n")
        };
        Self {
            date: today,
            overdue,
            today: todos.len() - overdue,
            text,
            todos,
        }
    }
}

impl AgendaTodo {
    /// `• text`, or `⚠ text (3d late)` when overdue.
    fn line(&self, today: NaiveDate) -> String {
        if self.overdue {
            let late = (today - self.due).num_days();
            format!("⚠ {} ({late}d late)", self.text)
        } else {
            format!("• {}", self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::server::test_app;

    /// GETs `uri`; returns the status and the body as text.
    async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
        let request = Request::get(uri)
            .body(Body::empty())
            .expect("valid request");
        let response = app.clone().oneshot(request).await.expect("infallible");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// A note with todos due around `today()`.
    fn note() -> String {
        let day = |offset: i64| today() + chrono::Duration::days(offset);
        format!(
            "- [ ] later 📅 {} ^aaaaaa\n\
             - [ ] today low 🔽 📅 {} ^bbbbbb\n\
             - [ ] today high ⏫ 📅 {} ^cccccc\n\
             - [/] late 📅 {} ^dddddd\n\
             - [x] late but done 📅 {} ✅ {} ^eeeeee\n\
             - [-] late but cancelled 📅 {} ^ffffff\n\
             - [ ] no date ^gggggg\n",
            day(1),
            day(0),
            day(0),
            day(-3),
            day(-3),
            day(0),
            day(-3),
        )
    }

    #[tokio::test]
    async fn json_lists_overdue_then_today() {
        let (_dir, app) = test_app(&note());
        let (status, body) = get(&app, "/api/agenda").await;
        assert_eq!(status, StatusCode::OK);
        let body: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(body["date"], json!(today()));
        assert_eq!(
            (body["overdue"].clone(), body["today"].clone()),
            (json!(1), json!(2))
        );
        assert_eq!(body["text"], "⚠ late (3d late)\n• today high\n• today low");
        let ids: Vec<&str> = body["todos"]
            .as_array()
            .expect("todos")
            .iter()
            .filter_map(|todo| todo["id"].as_str())
            .collect();
        assert_eq!(ids, ["dddddd", "cccccc", "bbbbbb"]);
        assert_eq!(body["todos"][0]["note"], "todo.md");
        assert_eq!(body["todos"][0]["overdue"], true);
        assert_eq!(body["todos"][1]["overdue"], false);
    }

    #[tokio::test]
    async fn text_format_is_the_lines_alone() {
        let (_dir, app) = test_app(&note());
        let (status, body) = get(&app, "/api/agenda?format=text").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "⚠ late (3d late)\n• today high\n• today low");

        let (_dir, app) = test_app("- [ ] no date ^aaaaaa\n");
        let (_, body) = get(&app, "/api/agenda?format=text").await;
        assert_eq!(body, "Nothing due");
        let (status, _) = get(&app, "/api/agenda?format=xml").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
