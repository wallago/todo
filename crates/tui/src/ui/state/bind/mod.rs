use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ListState,
};
use strum::IntoEnumIterator;

use crate::ui::prelude::Status;
use crate::{prelude::*, ui::prelude::*};

/// Dashboard mode's key handling.
mod dashboard;
/// Details mode's key handling.
mod details;
/// Key reference for the current focus and mode.
mod helper;
/// Input key handling.
mod input;

/// Shared pieces the per-mode handlers use.
mod prelude {
    pub(super) use super::{Action, neighbour, step_in};
}

/// What a key asks for, once bindings are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    /// Leave the app.
    Quit,
    /// Open the key reference.
    Help,
    /// Unbound key.
    Nothing,
    /// Move up or scroll up.
    Up,
    /// Move down or scroll down.
    Down,
    /// Move left.
    Left,
    /// Move right.
    Right,
    /// Next pane in tab order.
    NextPane,
    /// Previous pane in tab order.
    PrevPane,
    /// Confirm.
    Enter,
    /// Hand focus back to the sidebar.
    Leave,
    /// Create generated repo.
    Create,
    /// Up to the parent directory (oil's `-`).
    Parent,
    /// Back to the directory praline started in (oil's `_`).
    Start,
    /// Validate declaration in picker.
    Validate,
}

impl State {
    /// Resolves `key`: bindings from `config.toml` first, then fixed keys.
    fn action(&self, key: &KeyEvent) -> Action {
        let keys = &self.keybindings;
        let bound = [
            (&keys.quit, Action::Quit),
            (&keys.scroll_up, Action::Up),
            (&keys.scroll_down, Action::Down),
            (&keys.scroll_left, Action::Left),
            (&keys.scroll_right, Action::Right),
            (&keys.leave, Action::Leave),
            (&keys.confirm, Action::Enter),
            (&keys.create, Action::Create),
        ];
        if let Some(&(_, action)) = bound.iter().find(|(pattern, _)| pattern.matches(key)) {
            return action;
        }
        match key.code {
            KeyCode::Up => Action::Up,
            KeyCode::Down => Action::Down,
            KeyCode::Tab => Action::NextPane,
            KeyCode::BackTab => Action::PrevPane,
            KeyCode::Char('-') => Action::Parent,
            KeyCode::Char('_') => Action::Start,
            KeyCode::Char('v') => Action::Validate,
            KeyCode::Char('?') => Action::Help,
            _ => Action::Nothing,
        }
    }

    /// Runs `action` against the panes.
    fn on_action(&mut self, action: Action) -> Result<()> {
        match (self.focus, action) {
            (_, Action::Create) => {
                self.app.create()?;
                let target = self.app.dest.join(&self.app.name);
                self.status = Some(Status::Info(format!("created `{}`", target.display())));
            }
            (_, Action::Quit) => self.running = false,
            (Focus::Sidebar, Action::Up) => self.mode = self.mode.previous(),
            (Focus::Sidebar, Action::Down) => self.mode = self.mode.next(),
            (Focus::Sidebar, Action::NextPane | Action::PrevPane) => {
                let forward = action == Action::NextPane;
                let entered = match self.mode {
                    Mode::Dashboard => {
                        self.dashboard.focus = edge(forward);
                        true
                    }
                    Mode::Details => {
                        self.details.focus = edge(forward);
                        true
                    }
                    Mode::Settings => false,
                };
                if entered {
                    self.focus = Focus::Page;
                }
            }
            (Focus::Sidebar, Action::Enter) => self.focus = Focus::Page,
            (Focus::Page, Action::Leave) => self.focus = Focus::Sidebar,
            (Focus::Page, Action::NextPane | Action::PrevPane) => {
                let forward = action == Action::NextPane;
                let stayed = match self.mode {
                    Mode::Dashboard => self.dashboard.step_pane(forward),
                    Mode::Details => self.details.step_pane(forward),
                    Mode::Settings => false,
                };
                if !stayed {
                    self.focus = Focus::Sidebar;
                }
            }
            (Focus::Page, _) => match self.mode {
                Mode::Dashboard => self.dashboard.on_action(action, &mut self.app)?,
                Mode::Details => self.details.on_action(action, &mut self.app),
                Mode::Settings => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Handles a key press. Any key clears the last status; a failed action
    /// sets a new one instead of leaving the app.
    pub(crate) fn on_key(&mut self, key: KeyEvent) {
        self.status = None;
        if let Err(error) = self.dispatch(key) {
            self.status = Some(Status::Error(error.to_string()));
        }
    }

    /// Routes `key` to whatever owns the keyboard right now.
    fn dispatch(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == self.keybindings.quit.code {
            self.running = false;
            return Ok(());
        }
        if self.dashboard.editing.is_some() {
            return self.on_edit_key(key);
        }
        if self.dashboard.picker.is_some() {
            let action = self.action(&key);
            self.dashboard.on_picker_action(action, &mut self.app);
            return Ok(());
        }
        self.on_action(self.action(&key))
    }
}

/// Moves `table`'s selection one row within `len` rows, without wrapping.
fn step_in(table: &mut ListState, len: usize, down: bool) {
    let at = table.selected().unwrap_or(0);
    let next = if down { at + 1 } else { at.saturating_sub(1) };
    table.select(Some(next.min(len.saturating_sub(1))));
}

/// Next (or previous) variant of `current` in declaration order; `None` past either end.
fn neighbour<T: IntoEnumIterator + PartialEq + Copy>(current: T, forward: bool) -> Option<T> {
    let all: Vec<T> = T::iter().collect();
    let at = all.iter().position(|p| *p == current)?;
    let to = if forward { at + 1 } else { at.checked_sub(1)? };
    all.get(to).copied()
}

/// First variant when entering forward, last when entering backward.
fn edge<T: IntoEnumIterator + Default>(forward: bool) -> T {
    if forward {
        T::iter().next()
    } else {
        T::iter().last()
    }
    .unwrap_or_default()
}
