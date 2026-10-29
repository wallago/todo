//! What the UI shows, and how keys change it.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ListState,
};

/// Which side owns the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Focus {
    /// Page navigation on the left.
    #[default]
    Sidebar,
    /// The current page.
    Page,
}

/// Page shown on the right, picked in the sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Page {
    /// A list to move through.
    #[default]
    Home,
    /// Name, version and description of the crate.
    About,
}

impl Page {
    /// Every page, in sidebar order.
    pub(super) const ALL: [Self; 2] = [Self::Home, Self::About];

    /// Label in the sidebar.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::About => "about",
        }
    }

    /// Position in [`Self::ALL`].
    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|&page| page == self).unwrap_or(0)
    }

    /// The page after (or before) this one, or `self` past either end.
    fn step(self, forward: bool) -> Self {
        let at = self.index();
        let to = if forward {
            at.saturating_add(1)
        } else {
            at.saturating_sub(1)
        };
        Self::ALL.get(to).copied().unwrap_or(self)
    }
}

/// Application state.
#[derive(Debug)]
pub(super) struct State {
    /// Cleared to leave the event loop.
    pub(super) running: bool,
    /// Which side owns the keyboard.
    pub(super) focus: Focus,
    /// Page shown and selected in the sidebar.
    pub(super) page: Page,
    /// Rows of the home page's list.
    pub(super) items: Vec<&'static str>,
    /// Selected row of [`Self::items`].
    pub(super) list: ListState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running: true,
            focus: Focus::default(),
            page: Page::default(),
            items: vec!["first item", "second item", "third item"],
            list: ListState::default().with_selected(Some(0)),
        }
    }
}

impl State {
    /// Applies a key press.
    pub(super) fn on_key(&mut self, key: KeyEvent) {
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
            (Focus::Sidebar, _) if up || down => self.page = self.page.step(down),
            (Focus::Sidebar, KeyCode::Tab | KeyCode::Enter) => self.focus = Focus::Page,
            (Focus::Page, KeyCode::Tab | KeyCode::Esc) => self.focus = Focus::Sidebar,
            (Focus::Page, _) if (up || down) && self.page == Page::Home => {
                let at = self.list.selected().unwrap_or(0);
                let to = if down {
                    at.saturating_add(1)
                } else {
                    at.saturating_sub(1)
                };
                self.list
                    .select(Some(to.min(self.items.len().saturating_sub(1))));
            }
            _ => {}
        }
    }

    /// Keys that do something right now, as `(keys, what they do)`.
    pub(super) fn key_bindings(&self) -> &'static [(&'static str, &'static str)] {
        match self.focus {
            Focus::Sidebar => &[("↑↓", "page"), ("tab", "enter"), ("q", "quit")],
            Focus::Page => &[("↑↓", "move"), ("tab", "back"), ("q", "quit")],
        }
    }
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

    #[rstest]
    #[case::q(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))]
    #[case::ctrl_c(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))]
    fn quits(#[case] key: KeyEvent) {
        let mut state = State::default();
        state.on_key(key);
        assert!(!state.running);
    }

    #[test]
    fn sidebar_picks_the_page_without_wrapping() {
        let mut state = State::default();
        state.on_key(press(KeyCode::Down));
        assert_eq!(state.page, Page::About);
        state.on_key(press(KeyCode::Down));
        assert_eq!(state.page, Page::About);
        state.on_key(press(KeyCode::Up));
        assert_eq!(state.page, Page::Home);
    }

    #[test]
    fn page_list_stops_at_the_last_item() {
        let mut state = State::default();
        state.on_key(press(KeyCode::Tab));
        assert_eq!(state.focus, Focus::Page);
        for _ in 0..5 {
            state.on_key(press(KeyCode::Down));
        }
        assert_eq!(state.list.selected(), Some(state.items.len() - 1));
        assert_eq!(state.page, Page::Home);
        state.on_key(press(KeyCode::Esc));
        assert_eq!(state.focus, Focus::Sidebar);
    }
}
