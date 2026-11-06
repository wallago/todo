//! The dashboard page: totals and every due todo, across all notes.

use ratatui::widgets::ListState;
use todo_client::Todo;

use super::manager::step;

/// An open todo with a due date, and the note it lives in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::tui) struct DueTodo {
    /// Note path, like `projects/todo.md`.
    pub(in crate::tui) note: String,
    /// The todo.
    pub(in crate::tui) todo: Todo,
}

/// Counts and due todos of the whole vault.
#[derive(Debug, Default)]
pub(in crate::tui) struct Dashboard {
    /// Done or cancelled todos.
    pub(in crate::tui) done: usize,
    /// Todos still open.
    pub(in crate::tui) open: usize,
    /// Open todos with a due date, soonest first.
    pub(in crate::tui) due: Vec<DueTodo>,
    /// Selected row of [`Self::due`].
    pub(in crate::tui) list: ListState,
}

impl Dashboard {
    /// Recounts from every note's todos, keeping the cursor row in range.
    pub(in crate::tui) fn set(&mut self, notes: Vec<(String, Vec<Todo>)>) {
        self.done = 0;
        self.open = 0;
        self.due.clear();
        for (note, todos) in notes {
            for todo in todos {
                if todo.status.is_closed() {
                    self.done += 1;
                    continue;
                }
                self.open += 1;
                if todo.due.is_some() {
                    self.due.push(DueTodo {
                        note: note.clone(),
                        todo,
                    });
                }
            }
        }
        self.due
            .sort_by(|a, b| (a.todo.due, &a.note).cmp(&(b.todo.due, &b.note)));
        let at = self.list.selected().unwrap_or(0);
        self.list.select(if self.due.is_empty() {
            None
        } else {
            Some(at.min(self.due.len() - 1))
        });
    }

    /// The due todo under the cursor.
    pub(in crate::tui) fn selected(&self) -> Option<&DueTodo> {
        self.list.selected().and_then(|at| self.due.get(at))
    }

    /// Moves the cursor one row, without wrapping.
    pub(in crate::tui) fn step(&mut self, down: bool) {
        step(&mut self.list, self.due.len(), down);
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use todo_client::{Id, Status};

    use super::*;

    /// A todo due `due` days into November, or never.
    fn todo(id: &str, due: Option<u32>, status: Status) -> Todo {
        let mut todo = Todo::new(Id(id.into()), id, NaiveDate::MIN);
        todo.due = due.and_then(|day| NaiveDate::from_ymd_opt(2026, 11, day));
        todo.status = status;
        todo
    }

    #[test]
    fn counts_and_sorts_due_todos_across_notes() {
        let mut dashboard = Dashboard::default();
        dashboard.set(vec![
            (
                "a.md".into(),
                vec![
                    todo("late", Some(9), Status::Todo),
                    todo("closed", Some(1), Status::Done),
                    todo("undated", None, Status::Todo),
                ],
            ),
            ("b.md".into(), vec![todo("soon", Some(2), Status::Todo)]),
        ]);
        assert_eq!((dashboard.done, dashboard.open), (1, 3));
        let ids: Vec<_> = dashboard
            .due
            .iter()
            .map(|due| due.todo.text.as_str())
            .collect();
        assert_eq!(ids, ["soon", "late"]);
        assert_eq!(
            dashboard.selected().map(|due| due.note.as_str()),
            Some("b.md")
        );
    }
}
