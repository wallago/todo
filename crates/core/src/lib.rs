//! Core todo model: plain data, no I/O.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Stable identifier, written as an Obsidian block id (`^k3x9a2`).
///
/// Survives edits to the text, so the TUI, MCP and desk display can all
/// point at the same task.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

/// Checkbox state: the character between `[` and `]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// `[ ]`
    #[default]
    Todo,
    /// `[/]`
    InProgress,
    /// `[x]`
    Done,
    /// `[-]`
    Cancelled,
}

impl Status {
    /// The checkbox character Obsidian uses for this status.
    #[must_use]
    pub const fn as_char(self) -> char {
        match self {
            Self::Todo => ' ',
            Self::InProgress => '/',
            Self::Done => 'x',
            Self::Cancelled => '-',
        }
    }

    /// Reads a checkbox character. Unknown characters count as [`Status::Todo`]
    /// so a custom status in the vault never hides a task.
    #[must_use]
    pub const fn from_char(c: char) -> Self {
        match c {
            '/' => Self::InProgress,
            'x' | 'X' => Self::Done,
            '-' => Self::Cancelled,
            _ => Self::Todo,
        }
    }

    /// `true` once nothing is left to do.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

/// Tasks plugin priority. Ordered lowest to highest, so sorting works.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// `⏬`
    Lowest,
    /// `🔽`
    Low,
    /// No emoji.
    #[default]
    Normal,
    /// `🔼`
    Medium,
    /// `⏫`
    High,
    /// `🔺`
    Highest,
}

impl Priority {
    /// The emoji the Tasks plugin uses, or `None` for [`Priority::Normal`].
    #[must_use]
    pub const fn emoji(self) -> Option<&'static str> {
        match self {
            Self::Lowest => Some("⏬"),
            Self::Low => Some("🔽"),
            Self::Normal => None,
            Self::Medium => Some("🔼"),
            Self::High => Some("⏫"),
            Self::Highest => Some("🔺"),
        }
    }
}

/// One task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    /// Stable id (`^k3x9a2`).
    pub id: Id,
    /// What to do, without the emoji fields or tags.
    pub text: String,
    /// Checkbox state.
    #[serde(default)]
    pub status: Status,
    /// Priority emoji.
    #[serde(default)]
    pub priority: Priority,
    /// `📅` due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// `➕` created date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<NaiveDate>,
    /// `✅` done date, set when the task is completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<NaiveDate>,
    /// Tags without the leading `#`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Todo {
    /// A fresh open task.
    #[must_use]
    pub fn new(id: Id, text: impl Into<String>, today: NaiveDate) -> Self {
        Self {
            id,
            text: text.into(),
            status: Status::Todo,
            priority: Priority::Normal,
            due: None,
            created: Some(today),
            done: None,
            tags: Vec::new(),
        }
    }

    /// Ticks the box and stamps the done date. Calling it twice changes nothing,
    /// so a retrying client (desk display on flaky wifi) is safe.
    pub fn complete(&mut self, today: NaiveDate) {
        if self.status != Status::Done {
            self.status = Status::Done;
            self.done = Some(today);
        }
    }

    /// Unticks the box and clears the done date.
    pub fn reopen(&mut self) {
        self.status = Status::Todo;
        self.done = None;
    }

    /// `true` if still open and the due date is before `today`.
    #[must_use]
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        !self.status.is_closed() && self.due.is_some_and(|due| due < today)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Shorthand for building dates in tests.
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default()
    }

    #[test]
    fn complete_is_idempotent() {
        let mut todo = Todo::new(Id("a".into()), "buy milk", date(2026, 10, 24));
        todo.complete(date(2026, 10, 25));
        todo.complete(date(2026, 10, 26));
        assert_eq!(todo.status, Status::Done);
        assert_eq!(todo.done, Some(date(2026, 10, 25)));
        todo.reopen();
        assert_eq!(todo.done, None);
    }

    #[test]
    fn overdue_only_when_open() {
        let mut todo = Todo::new(Id("a".into()), "buy milk", date(2026, 10, 1));
        todo.due = Some(date(2026, 10, 20));
        assert!(todo.is_overdue(date(2026, 10, 24)));
        assert!(!todo.is_overdue(date(2026, 10, 20)));
        todo.complete(date(2026, 10, 24));
        assert!(!todo.is_overdue(date(2026, 10, 24)));
    }

    #[test]
    fn status_char_round_trips() {
        for status in [Status::Todo, Status::InProgress, Status::Done, Status::Cancelled] {
            assert_eq!(Status::from_char(status.as_char()), status);
        }
    }

    #[test]
    fn json_is_flat_and_skips_empty_fields() -> serde_json::Result<()> {
        let todo = Todo::new(Id("k3x9a2".into()), "buy milk", date(2026, 10, 24));
        let json = serde_json::to_string(&todo)?;
        assert_eq!(
            json,
            r#"{"id":"k3x9a2","text":"buy milk","status":"todo","priority":"normal","created":"2026-10-24"}"#
        );
        assert_eq!(serde_json::from_str::<Todo>(&json)?, todo);
        Ok(())
    }
}
