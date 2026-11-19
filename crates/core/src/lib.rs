//! Core todo model: plain data, no I/O.

use chrono::{Days, Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// Stable identifier: a UUID, or the Obsidian block id (`k3x9a2`) of a todo
/// imported from a vault. Treat it as an opaque string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Id(pub String);

/// Checkbox state: the character between `[` and `]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

/// Unit of a [`Recur`] interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Days.
    Day,
    /// Weeks.
    Week,
    /// Months; the 31st becomes the month's last day.
    Month,
    /// Years.
    Year,
}

/// How often a todo comes back: every `every` `unit`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Recur {
    /// How many units between two occurrences, at least 1.
    #[serde(default = "one")]
    pub every: u32,
    /// Day, week, month or year.
    pub unit: Unit,
}

/// Default for [`Recur::every`].
const fn one() -> u32 {
    1
}

impl Recur {
    /// The date one interval after `from`. Stays on `from` if it would
    /// overflow, which no real date does.
    #[must_use]
    pub fn next(self, from: NaiveDate) -> NaiveDate {
        let n = self.every.max(1);
        let next = match self.unit {
            Unit::Day => from.checked_add_days(Days::new(n.into())),
            Unit::Week => from.checked_add_days(Days::new(u64::from(n) * 7)),
            Unit::Month => from.checked_add_months(Months::new(n)),
            Unit::Year => from.checked_add_months(Months::new(n.saturating_mul(12))),
        };
        next.unwrap_or(from)
    }

    /// The todo that comes after `done`, completed on `today`: same fields,
    /// open, due one interval after its due date (or after `today`).
    /// Its id is empty, for the store to fill.
    #[must_use]
    pub fn follow_up(self, done: &Todo, today: NaiveDate) -> Todo {
        let mut next = Todo::new(Id(String::new()), done.text.clone(), today);
        next.priority = done.priority;
        next.tags.clone_from(&done.tags);
        next.pinned = done.pinned;
        next.recur = Some(self);
        next.due = Some(self.next(done.due.unwrap_or(today)));
        next
    }
}

/// One task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
    /// Being worked on, usually with no due date: shown first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Comes back when completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recur: Option<Recur>,
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
            pinned: false,
            recur: None,
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

/// What changed since a sync cursor: answer of `GET /api/sync`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    /// Send it as `since` next time to get only what changed after this.
    pub cursor: u64,
    /// Notes created or deleted since.
    #[serde(default)]
    pub notes: Vec<NoteChange>,
    /// Todos added, changed or deleted since.
    #[serde(default)]
    pub todos: Vec<TodoChange>,
}

/// A note in [`Changes`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteChange {
    /// Like `projects/dev`.
    pub name: String,
    /// Gone: forget it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

/// A todo in [`Changes`], as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoChange {
    /// The note it's in.
    pub note: String,
    /// Gone: forget it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The todo.
    #[serde(flatten)]
    pub todo: Todo,
}

/// Rewrites `"type": [T, "null"]` (how schemars spells `Option<T>`) as
/// `anyOf: [{"type": T}, {"type": "null"}]`, which more MCP clients accept.
///
/// Use as `#[schemars(transform = todo_core::nullable_any_of)]`.
#[cfg(feature = "schemars")]
pub fn nullable_any_of(schema: &mut schemars::Schema) {
    use serde_json::{Map, Value, json};

    /// Keys that describe the field, so they stay outside the `anyOf`.
    const META: [&str; 7] = [
        "description",
        "title",
        "default",
        "examples",
        "deprecated",
        "readOnly",
        "writeOnly",
    ];

    schemars::transform::transform_subschemas(&mut nullable_any_of, schema);
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    let Some(Value::Array(types)) = object.get("type") else {
        return;
    };
    let null = json!("null");
    let [first, second] = types.as_slice() else {
        return;
    };
    let inner = match (first == &null, second == &null) {
        (false, true) => first.clone(),
        (true, false) => second.clone(),
        _ => return,
    };
    object.remove("type");
    let mut branch = Map::new();
    branch.insert("type".to_owned(), inner);
    let moved: Vec<String> = object
        .keys()
        .filter(|key| !META.contains(&key.as_str()) && !key.starts_with('$'))
        .cloned()
        .collect();
    for key in moved {
        if let Some(value) = object.remove(&key) {
            branch.insert(key, value);
        }
    }
    object.insert(
        "anyOf".to_owned(),
        json!([Value::Object(branch), {"type": "null"}]),
    );
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
    fn recur_moves_the_due_date() {
        let every = |every, unit| Recur { every, unit };
        assert_eq!(
            every(2, Unit::Day).next(date(2026, 12, 31)),
            date(2027, 1, 2)
        );
        assert_eq!(
            every(1, Unit::Week).next(date(2026, 11, 19)),
            date(2026, 11, 26)
        );
        assert_eq!(
            every(1, Unit::Month).next(date(2026, 1, 31)),
            date(2026, 2, 28)
        );
        assert_eq!(
            every(1, Unit::Year).next(date(2028, 2, 29)),
            date(2029, 2, 28)
        );
        assert_eq!(every(0, Unit::Day).next(date(2026, 1, 1)), date(2026, 1, 2));
    }

    #[test]
    fn follow_up_copies_and_moves_on() {
        let recur = Recur {
            every: 1,
            unit: Unit::Week,
        };
        let mut todo = Todo::new(Id("a".into()), "water plants", date(2026, 1, 1));
        todo.tags = vec!["home".into()];
        todo.pinned = true;
        todo.due = Some(date(2026, 11, 10));
        todo.complete(date(2026, 11, 19));
        let next = recur.follow_up(&todo, date(2026, 11, 19));
        assert_eq!(next.status, Status::Todo);
        assert_eq!(next.due, Some(date(2026, 11, 17)));
        assert_eq!(next.created, Some(date(2026, 11, 19)));
        assert_eq!(
            (&next.tags, next.pinned, next.recur),
            (&todo.tags, true, Some(recur))
        );
        todo.due = None;
        let next = recur.follow_up(&todo, date(2026, 11, 19));
        assert_eq!(next.due, Some(date(2026, 11, 26)));
    }

    #[test]
    fn status_char_round_trips() {
        for status in [
            Status::Todo,
            Status::InProgress,
            Status::Done,
            Status::Cancelled,
        ] {
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
