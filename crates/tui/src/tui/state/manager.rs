//! The manager page: one note's todos.

use ratatui::widgets::ListState;
use todo_client::{Id, Status, Todo};

/// The note open on the manager page, and its todos.
#[derive(Debug, Default)]
pub(in crate::tui) struct Manager {
    /// Open note, like `projects/todo.md`.
    pub(in crate::tui) note: Option<String>,
    /// Its todos.
    pub(in crate::tui) todos: Vec<Todo>,
    /// Selected row of [`Self::todos`].
    pub(in crate::tui) list: ListState,
}

impl Manager {
    /// Switches to `note`; its todos come with the next load.
    pub(in crate::tui) fn open(&mut self, note: Option<String>) {
        if note != self.note {
            self.note = note;
            self.todos.clear();
            self.list.select(None);
        }
    }

    /// Replaces the todos, sorted, keeping the cursor row in range.
    pub(in crate::tui) fn set_todos(&mut self, mut todos: Vec<Todo>) {
        sort(&mut todos);
        let at = self.list.selected().unwrap_or(0);
        self.list.select(if todos.is_empty() {
            None
        } else {
            Some(at.min(todos.len() - 1))
        });
        self.todos = todos;
    }

    /// The todo under the cursor.
    pub(in crate::tui) fn selected(&self) -> Option<&Todo> {
        self.list.selected().and_then(|at| self.todos.get(at))
    }

    /// Puts `todo` in place of the one with the same id.
    pub(in crate::tui) fn replace(&mut self, todo: Todo) {
        if let Some(old) = self.todos.iter_mut().find(|old| old.id == todo.id) {
            *old = todo;
            // The cursor stays on its row, so ticking moves on to the next.
            sort(&mut self.todos);
        }
    }

    /// Puts the cursor on the todo with `id`, if it's there.
    pub(in crate::tui) fn select(&mut self, id: &Id) {
        if let Some(at) = self.todos.iter().position(|todo| &todo.id == id) {
            self.list.select(Some(at));
        }
    }

    /// Moves the cursor one row, without wrapping.
    pub(in crate::tui) fn step(&mut self, down: bool) {
        step(&mut self.list, self.todos.len(), down);
    }

    /// Done or cancelled todos, and every todo.
    pub(in crate::tui) fn counts(&self) -> (usize, usize) {
        let done = self
            .todos
            .iter()
            .filter(|todo| todo.status.is_closed())
            .count();
        (done, self.todos.len())
    }
}

/// To do, in progress, cancelled, then done; soonest due first in each,
/// undated last, file order otherwise.
fn sort(todos: &mut [Todo]) {
    todos.sort_by_key(|todo| {
        let rank = match todo.status {
            Status::Todo => 0,
            Status::InProgress => 1,
            Status::Cancelled => 2,
            Status::Done => 3,
        };
        (rank, todo.due.is_none(), todo.due)
    });
}

/// Moves `list` one row, without wrapping, in a list of `len` rows.
pub(super) fn step(list: &mut ListState, len: usize, down: bool) {
    if len == 0 {
        return;
    }
    let at = list.selected().unwrap_or(0);
    let to = if down {
        at.saturating_add(1)
    } else {
        at.saturating_sub(1)
    };
    list.select(Some(to.min(len - 1)));
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    use super::*;

    /// A todo with `status`, due `day` of November or never.
    fn todo(id: &str, status: Status, day: Option<u32>) -> Todo {
        let mut todo = Todo::new(Id(id.into()), id, NaiveDate::MIN);
        todo.status = status;
        todo.due = day.and_then(|day| NaiveDate::from_ymd_opt(2026, 11, day));
        todo
    }

    /// The todos' texts, in order.
    fn order(manager: &Manager) -> Vec<&str> {
        manager
            .todos
            .iter()
            .map(|todo| todo.text.as_str())
            .collect()
    }

    #[test]
    fn sorts_by_status_then_due_date() {
        let mut manager = Manager::default();
        manager.set_todos(vec![
            todo("done", Status::Done, Some(1)),
            todo("undated", Status::Todo, None),
            todo("later", Status::Todo, Some(6)),
            todo("cancelled", Status::Cancelled, None),
            todo("doing", Status::InProgress, Some(9)),
            todo("soon", Status::Todo, Some(3)),
        ]);
        assert_eq!(
            order(&manager),
            ["soon", "later", "undated", "doing", "cancelled", "done"]
        );
    }

    #[test]
    fn a_ticked_todo_moves_down_and_the_cursor_stays() {
        let mut manager = Manager::default();
        manager.set_todos(vec![
            todo("a", Status::Todo, Some(1)),
            todo("b", Status::Todo, Some(2)),
        ]);
        manager.replace(todo("a", Status::Done, Some(1)));
        assert_eq!(order(&manager), ["b", "a"]);
        assert_eq!(manager.selected().map(|todo| todo.text.as_str()), Some("b"));
    }
}
