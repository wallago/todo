//! The manager page: one note's todos.

use ratatui::widgets::ListState;
use todo_client::Todo;

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

    /// Replaces the todos, keeping the cursor row in range.
    pub(in crate::tui) fn set_todos(&mut self, todos: Vec<Todo>) {
        let at = self.list.selected().unwrap_or(0);
        self.list.select(if todos.is_empty() {
            None
        } else {
            Some(at.min(todos.len() - 1))
        });
        self.todos = todos;
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
