//! The note finder popup.

use nucleo_matcher::{
    Config, Matcher,
    pattern::{CaseMatching, Normalization, Pattern},
};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ListState,
};

use super::manager::step;
use crate::config::Keys;

/// A query, and the notes it fuzzy-matches, best first.
#[derive(Debug)]
pub(in crate::tui) struct Picker {
    /// Every note, to filter.
    notes: Vec<String>,
    /// What was typed.
    pub(in crate::tui) query: String,
    /// Notes matching [`Self::query`], best first.
    pub(in crate::tui) matches: Vec<String>,
    /// Selected row of [`Self::matches`].
    pub(in crate::tui) list: ListState,
}

impl Picker {
    /// A picker over `notes`, all of them shown.
    pub(in crate::tui) fn new(notes: Vec<String>) -> Self {
        let mut picker = Self {
            notes,
            query: String::new(),
            matches: Vec::new(),
            list: ListState::default(),
        };
        picker.filter();
        picker
    }

    /// The selected note, if anything matches.
    pub(in crate::tui) fn selected(&self) -> Option<&str> {
        self.list
            .selected()
            .and_then(|at| self.matches.get(at))
            .map(String::as_str)
    }

    /// Edits the query or moves the cursor. Plain letters type, so the
    /// movement keys from `keys` need Alt or Ctrl here.
    pub(in crate::tui) fn on_key(&mut self, key: KeyEvent, keys: Keys) {
        let held = key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char(c) if held && c == keys.up => {
                step(&mut self.list, self.matches.len(), false);
            }
            KeyCode::Char(c) if held && c == keys.down => {
                step(&mut self.list, self.matches.len(), true);
            }
            KeyCode::Up => step(&mut self.list, self.matches.len(), false),
            KeyCode::Down => step(&mut self.list, self.matches.len(), true),
            KeyCode::Backspace => {
                self.query.pop();
                self.filter();
            }
            KeyCode::Char(c) if !held => {
                self.query.push(c);
                self.filter();
            }
            _ => {}
        }
    }

    /// Recomputes [`Self::matches`] and puts the cursor on the best one.
    fn filter(&mut self) {
        let mut config = Config::DEFAULT;
        config.set_match_paths();
        let mut matcher = Matcher::new(config);
        let pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);
        self.matches = pattern
            .match_list(&self.notes, &mut matcher)
            .into_iter()
            .map(|(note, _)| note.clone())
            .collect();
        self.list.select((!self.matches.is_empty()).then_some(0));
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Presses each character of `text`.
    fn type_text(picker: &mut Picker, text: &str) {
        for c in text.chars() {
            picker.on_key(
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
                Keys::default(),
            );
        }
    }

    #[test]
    fn alt_with_a_movement_key_moves_instead_of_typing() {
        let mut picker = Picker::new(vec!["a.md".into(), "b.md".into()]);
        let keys = Keys {
            up: 'i',
            down: 'e',
            ..Keys::default()
        };
        picker.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT), keys);
        assert_eq!(picker.selected(), Some("b.md"));
        picker.on_key(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
            keys,
        );
        assert_eq!(picker.selected(), Some("a.md"));
        assert_eq!(picker.query, "");
    }

    #[test]
    fn filters_as_you_type() {
        let mut picker = Picker::new(vec![
            "projects/todo/main.md".into(),
            "todo.md".into(),
            "travel/japan.md".into(),
        ]);
        assert_eq!(picker.matches.len(), 3);
        type_text(&mut picker, "tdmain");
        assert_eq!(picker.matches, vec!["projects/todo/main.md".to_owned()]);
        assert_eq!(picker.selected(), Some("projects/todo/main.md"));
        type_text(&mut picker, "zzz");
        assert_eq!(picker.selected(), None);
    }
}
