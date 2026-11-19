//! What goes over the wire between the server and its clients (TUI, MCP).
//!
//! Responses are plain [`Todo`]s; this crate adds the request bodies and,
//! with the `http` feature (on by default), a [`Client`] to send them.
//!
//! ```no_run
//! # async fn demo() -> todo_client::Result<()> {
//! use todo_client::{Client, NewTodo};
//!
//! let client = Client::new("http://127.0.0.1:3000");
//! let todo = client.add(&NewTodo { text: "buy milk".into(), note: "errands".into(), ..NewTodo::default() }).await?;
//! client.complete(&todo.id).await?;
//! # Ok(())
//! # }
//! ```

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
pub use todo_core::{Changes, Id, NoteChange, Priority, Recur, Status, Todo, TodoChange, Unit};

/// Request functions.
#[cfg(feature = "http")]
mod http;

#[cfg(feature = "http")]
pub use http::{Client, Error, Result};

/// Body of `POST /api/todos`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(transform = todo_core::nullable_any_of))]
pub struct NewTodo {
    /// Id chosen by the client, a UUID, so an offline client can send the
    /// same add twice safely: the second answers the todo already saved.
    /// The server makes one if left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// What to do.
    pub text: String,
    /// Priority, normal if left out.
    #[serde(default)]
    pub priority: Priority,
    /// Due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// Tags without the leading `#`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Note to add it to, like `projects/dev`. Created if missing.
    pub note: String,
    /// Pin it: being worked on, usually with no due date.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Make it come back when completed, e.g. `{"every": 1, "unit": "week"}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recur: Option<Recur>,
}

/// Query of `GET /api/todos`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoQuery {
    /// Only todos in this note, like `travel/japan`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Query of `GET /api/sync`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncQuery {
    /// The `cursor` of the last sync; 0 (or left out) for everything.
    #[serde(default)]
    pub since: u64,
}

/// Body of `PATCH /api/todos/{id}`. Fields left out stay as they are.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(transform = todo_core::nullable_any_of))]
pub struct TodoPatch {
    /// New text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// New status. `done` stamps the done date, `todo` clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// New priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// New due date; `null` clears it.
    #[serde(
        default,
        deserialize_with = "de::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub due: Option<Option<NaiveDate>>,
    /// New tags, replacing the old ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Pin or unpin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    /// New recurrence; `null` stops it.
    #[serde(
        default,
        deserialize_with = "de::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub recur: Option<Option<Recur>>,
}

impl TodoPatch {
    /// Applies the fields that are set to `todo`. Status goes through
    /// `complete`/`reopen` so the done date stays right. The server and
    /// offline clients both use it, so they agree.
    pub fn apply(self, todo: &mut Todo, today: NaiveDate) {
        if let Some(text) = self.text {
            text.trim().clone_into(&mut todo.text);
        }
        if let Some(priority) = self.priority {
            todo.priority = priority;
        }
        if let Some(due) = self.due {
            todo.due = due;
        }
        if let Some(tags) = self.tags {
            todo.tags = tags;
        }
        if let Some(pinned) = self.pinned {
            todo.pinned = pinned;
        }
        if let Some(recur) = self.recur {
            todo.recur = recur;
        }
        match self.status {
            Some(Status::Done) => todo.complete(today),
            Some(Status::Todo) => todo.reopen(),
            Some(status) => {
                todo.status = status;
                todo.done = None;
            }
            None => {}
        }
    }
}

impl NewTodo {
    /// The todo this adds, as the server saves it: trimmed text, created
    /// `today`, open. Its id is `self.id`, or empty for the server to fill.
    #[must_use]
    pub fn to_todo(&self, today: NaiveDate) -> Todo {
        let mut todo = Todo::new(
            self.id.clone().unwrap_or(Id(String::new())),
            self.text.trim(),
            today,
        );
        todo.priority = self.priority;
        todo.due = self.due;
        todo.tags.clone_from(&self.tags);
        todo.pinned = self.pinned;
        todo.recur = self.recur;
        todo
    }
}

/// Body of every error response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// What went wrong.
    pub error: String,
}

/// Serde helpers, apart so they see the std `Result`.
mod de {
    use serde::{Deserialize, Deserializer};

    /// Tells a field sent as `null` (`Some(None)`) from a missing one (`None`).
    pub(crate) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        T::deserialize(deserializer).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn patch_tells_null_from_missing() {
        let keep: TodoPatch = serde_json::from_str("{}").expect("json");
        assert_eq!(keep.due, None);
        let clear: TodoPatch = serde_json::from_str(r#"{"due": null}"#).expect("json");
        assert_eq!(clear.due, Some(None));
        let set: TodoPatch = serde_json::from_str(r#"{"due": "2026-10-30"}"#).expect("json");
        assert_eq!(set.due, Some(NaiveDate::from_ymd_opt(2026, 10, 30)));
        // And back: clearing is sent as null, keeping is left out.
        assert_eq!(
            serde_json::to_string(&clear).expect("json"),
            r#"{"due":null}"#
        );
        assert_eq!(serde_json::to_string(&keep).expect("json"), "{}");
    }

    #[test]
    fn new_todo_needs_only_text_and_note() {
        let new: NewTodo =
            serde_json::from_str(r#"{"text": "buy milk", "note": "errands"}"#).expect("json");
        assert_eq!(
            (new.text.as_str(), new.note.as_str()),
            ("buy milk", "errands")
        );
        assert_eq!(new.priority, Priority::Normal);
        assert!(serde_json::from_str::<NewTodo>(r#"{"text": "buy milk"}"#).is_err());
    }
}
