//! The one-line popup: add or change a todo, or confirm a delete.

use ratatui::crossterm::event::KeyCode;
use todo_client::Id;

/// What the popup is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::tui) enum Ask {
    /// Text of a new todo.
    Add,
    /// New text for the todo with this id.
    Change(Id),
    /// `y/n` before deleting the todo with this id.
    Delete(Id),
}

/// What a key did to the popup.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::tui) enum Outcome {
    /// Still open.
    Open,
    /// Closed without doing anything.
    Cancel,
    /// Closed with an answer.
    Submit,
}

/// The popup's purpose and what was typed.
#[derive(Clone, Debug)]
pub(in crate::tui) struct Prompt {
    /// What it's for.
    pub(in crate::tui) ask: Ask,
    /// Typed text; for [`Ask::Delete`], the todo's text, shown only.
    pub(in crate::tui) input: String,
}

impl Prompt {
    /// Edits the text, or answers `y/n` for a delete.
    pub(in crate::tui) fn on_key(&mut self, code: KeyCode) -> Outcome {
        if matches!(self.ask, Ask::Delete(_)) {
            return match code {
                KeyCode::Char('y') | KeyCode::Enter => Outcome::Submit,
                KeyCode::Char('n') | KeyCode::Esc => Outcome::Cancel,
                _ => Outcome::Open,
            };
        }
        match code {
            KeyCode::Esc => Outcome::Cancel,
            // Nothing typed: nothing to save.
            KeyCode::Enter if self.input.trim().is_empty() => Outcome::Cancel,
            KeyCode::Enter => Outcome::Submit,
            KeyCode::Backspace => {
                self.input.pop();
                Outcome::Open
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                Outcome::Open
            }
            _ => Outcome::Open,
        }
    }

    /// Title of the popup.
    pub(in crate::tui) fn title(&self) -> &'static str {
        match self.ask {
            Ask::Add => "new todo",
            Ask::Change(_) => "change todo",
            Ask::Delete(_) => "delete todo? y/n",
        }
    }
}
