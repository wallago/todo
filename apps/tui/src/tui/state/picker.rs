//! The topic finder popup.

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

/// A query, and the topics it fuzzy-matches, best first.
#[derive(Debug)]
pub(in crate::tui) struct Picker {
    /// Every topic, to filter.
    topics: Vec<String>,
    /// What was typed.
    pub(in crate::tui) query: String,
    /// Topics matching [`Self::query`], best first.
    pub(in crate::tui) matches: Vec<String>,
    /// Selected row of [`Self::matches`].
    pub(in crate::tui) list: ListState,
}

impl Picker {
    /// A picker over `topics`, all of them shown.
    pub(in crate::tui) fn new(topics: Vec<String>) -> Self {
        let mut picker = Self {
            topics,
            query: String::new(),
            matches: Vec::new(),
            list: ListState::default(),
        };
        picker.filter();
        picker
    }

    /// The selected topic, if anything matches.
    pub(in crate::tui) fn selected(&self) -> Option<&str> {
        self.list
            .selected()
            .and_then(|at| self.matches.get(at))
            .map(String::as_str)
    }

    /// The topic to open: the selected match, or with none, the typed
    /// path as a new topic.
    pub(in crate::tui) fn choice(&self) -> Option<String> {
        if let Some(topic) = self.selected() {
            return Some(topic.to_owned());
        }
        self.new_topic()
    }

    /// The typed path as a topic, when nothing matches it.
    pub(in crate::tui) fn new_topic(&self) -> Option<String> {
        let path = self.query.trim().trim_matches('/');
        if !self.matches.is_empty() || path.is_empty() {
            return None;
        }
        // Old habit: `travel/norway.md` is the topic `travel/norway`.
        Some(path.strip_suffix(".md").unwrap_or(path).to_owned())
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
            .match_list(&self.topics, &mut matcher)
            .into_iter()
            .map(|(topic, _)| topic.clone())
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
        let mut picker = Picker::new(vec!["a".into(), "b".into()]);
        let keys = Keys {
            up: 'i',
            down: 'e',
            ..Keys::default()
        };
        picker.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT), keys);
        assert_eq!(picker.selected(), Some("b"));
        picker.on_key(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
            keys,
        );
        assert_eq!(picker.selected(), Some("a"));
        assert_eq!(picker.query, "");
    }

    #[test]
    fn filters_as_you_type() {
        let mut picker = Picker::new(vec![
            "projects/todo/main".into(),
            "todo".into(),
            "travel/japan".into(),
        ]);
        assert_eq!(picker.matches.len(), 3);
        type_text(&mut picker, "tdmain");
        assert_eq!(picker.matches, vec!["projects/todo/main".to_owned()]);
        assert_eq!(picker.selected(), Some("projects/todo/main"));
        type_text(&mut picker, "zzz");
        assert_eq!(picker.selected(), None);
    }

    #[test]
    fn unmatched_query_is_a_new_topic() {
        let mut picker = Picker::new(vec!["todo".into()]);
        assert_eq!(picker.new_topic(), None);
        type_text(&mut picker, "travel/norway");
        assert_eq!(picker.choice().as_deref(), Some("travel/norway"));
        type_text(&mut picker, ".md");
        assert_eq!(picker.choice().as_deref(), Some("travel/norway"));
    }
}
