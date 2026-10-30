//! The `y/n` popup before deleting a todo.

use ratatui::crossterm::event::KeyCode;
use todo_client::Id;

use super::Outcome;

/// The todo about to be deleted.
#[derive(Clone, Debug)]
pub(in crate::tui) struct Confirm {
    /// Its id.
    pub(in crate::tui) id: Id,
    /// Its text, shown in the popup.
    pub(in crate::tui) text: String,
}

impl Confirm {
    /// `y` or `enter` deletes, `n` or `esc` keeps it; other keys wait.
    pub(in crate::tui) fn on_key(code: KeyCode) -> Outcome {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => Outcome::Submit,
            KeyCode::Char('n') | KeyCode::Esc => Outcome::Cancel,
            _ => Outcome::Open,
        }
    }
}
