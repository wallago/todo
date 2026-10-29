//! What the UI shows, and how keys change it.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ListState,
};
use todo_client::Todo;

/// Which pane owns the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Focus {
    /// Notes on the left.
    #[default]
    Notes,
    /// Todos of the selected note.
    Todos,
}

/// Server calls a key asks for; the event loop runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    /// Reload the notes, then the selected note's todos.
    Refresh,
    /// Reload the selected note's todos.
    LoadTodos,
}

/// Application state.
#[derive(Debug)]
pub(super) struct State {
    /// Cleared to leave the event loop.
    pub(super) running: bool,
    /// Which pane owns the keyboard.
    pub(super) focus: Focus,
    /// Note paths from the server, like `projects/todo.md`.
    pub(super) notes: Vec<String>,
    /// Selected row of [`Self::notes`].
    pub(super) note_list: ListState,
    /// Todos of the selected note.
    pub(super) todos: Vec<Todo>,
    /// Selected row of [`Self::todos`].
    pub(super) todo_list: ListState,
    /// Last server error, shown until the next successful call.
    pub(super) error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running: true,
            focus: Focus::default(),
            notes: Vec::new(),
            note_list: ListState::default(),
            todos: Vec::new(),
            todo_list: ListState::default(),
            error: None,
        }
    }
}

impl State {
    /// The selected note, if any.
    pub(super) fn note(&self) -> Option<&str> {
        self.note_list
            .selected()
            .and_then(|at| self.notes.get(at))
            .map(String::as_str)
    }

    /// Replaces the notes, keeping the selected one if it's still there.
    pub(super) fn set_notes(&mut self, notes: Vec<String>) {
        let kept = self
            .note()
            .and_then(|note| notes.iter().position(|n| n == note));
        self.note_list
            .select(kept.or_else(|| (!notes.is_empty()).then_some(0)));
        self.notes = notes;
    }

    /// Replaces the todos, keeping the cursor row in range.
    pub(super) fn set_todos(&mut self, todos: Vec<Todo>) {
        let at = self.todo_list.selected().unwrap_or(0);
        self.todo_list.select(if todos.is_empty() {
            None
        } else {
            Some(at.min(todos.len() - 1))
        });
        self.todos = todos;
    }

    /// Applies a key press, returning the server call it needs, if any.
    pub(super) fn on_key(&mut self, key: KeyEvent) -> Option<Action> {
        let ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        let (up, down) = (
            matches!(key.code, KeyCode::Up | KeyCode::Char('k')),
            matches!(key.code, KeyCode::Down | KeyCode::Char('j')),
        );
        match (self.focus, key.code) {
            // Raw mode turns Ctrl-C into a key, so it never reaches the signal handler.
            _ if ctrl_c => self.running = false,
            (_, KeyCode::Char('q')) => self.running = false,
            (_, KeyCode::Char('r')) => return Some(Action::Refresh),
            (Focus::Notes, _) if up || down => {
                let before = self.note_list.selected();
                step(&mut self.note_list, self.notes.len(), down);
                if self.note_list.selected() != before {
                    // A new note starts at its first todo.
                    self.todo_list.select(None);
                    return Some(Action::LoadTodos);
                }
            }
            (Focus::Notes, KeyCode::Tab | KeyCode::Enter | KeyCode::Char('l')) => {
                self.focus = Focus::Todos;
            }
            (Focus::Todos, KeyCode::Tab | KeyCode::Esc | KeyCode::Char('h')) => {
                self.focus = Focus::Notes;
            }
            (Focus::Todos, _) if up || down => step(&mut self.todo_list, self.todos.len(), down),
            _ => {}
        }
        None
    }

    /// Keys that do something right now, as `(keys, what they do)`.
    pub(super) fn key_bindings(&self) -> &'static [(&'static str, &'static str)] {
        match self.focus {
            Focus::Notes => &[
                ("↑↓", "note"),
                ("tab", "todos"),
                ("r", "refresh"),
                ("q", "quit"),
            ],
            Focus::Todos => &[
                ("↑↓", "move"),
                ("tab", "notes"),
                ("r", "refresh"),
                ("q", "quit"),
            ],
        }
    }
}

/// Moves `list` one row, without wrapping, in a list of `len` rows.
fn step(list: &mut ListState, len: usize, down: bool) {
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
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    /// A press of `code` with no modifier.
    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// A state with two notes, the first selected.
    fn with_notes() -> State {
        let mut state = State::default();
        state.set_notes(vec!["a.md".into(), "b.md".into()]);
        state
    }

    #[rstest]
    #[case::q(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))]
    #[case::ctrl_c(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))]
    fn quits(#[case] key: KeyEvent) {
        let mut state = State::default();
        state.on_key(key);
        assert!(!state.running);
    }

    #[test]
    fn r_refreshes() {
        assert_eq!(
            State::default().on_key(press(KeyCode::Char('r'))),
            Some(Action::Refresh)
        );
    }

    #[test]
    fn moving_to_another_note_loads_its_todos() {
        let mut state = with_notes();
        assert_eq!(state.note(), Some("a.md"));
        assert_eq!(state.on_key(press(KeyCode::Down)), Some(Action::LoadTodos));
        assert_eq!(state.note(), Some("b.md"));
        // Past the end: nothing changes, nothing to load.
        assert_eq!(state.on_key(press(KeyCode::Down)), None);
        assert_eq!(state.note(), Some("b.md"));
    }

    #[test]
    fn refreshed_notes_keep_the_selection() {
        let mut state = with_notes();
        state.on_key(press(KeyCode::Down));
        state.set_notes(vec!["0.md".into(), "a.md".into(), "b.md".into()]);
        assert_eq!(state.note(), Some("b.md"));
        state.set_notes(vec!["c.md".into()]);
        assert_eq!(state.note(), Some("c.md"));
    }

    #[test]
    fn tab_switches_panes() {
        let mut state = with_notes();
        state.on_key(press(KeyCode::Tab));
        assert_eq!(state.focus, Focus::Todos);
        state.on_key(press(KeyCode::Esc));
        assert_eq!(state.focus, Focus::Notes);
    }
}
