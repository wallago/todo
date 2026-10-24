use super::prelude::*;
use crate::{app::App, ui::prelude::*};

impl Details {
    /// Runs `action` against the focused pane.
    pub(super) fn on_action(&mut self, action: Action, app: &mut App) {
        let len = app.staged.as_ref().map_or(
            0,
            |tree: &std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>| tree.len(),
        );
        match (self.focus, action) {
            (DetailsPane::List, Action::Up | Action::Down) => {
                step_in(&mut self.files, len, action == Action::Down);
                self.scroll = 0;
            }
            (DetailsPane::Content, Action::Up) => self.scroll = self.scroll.saturating_sub(1),
            (DetailsPane::Content, Action::Down) => self.scroll = self.scroll.saturating_add(1),
            _ => {}
        }
    }

    /// Moves focus to the neighbouring pane; `false` when there is none.
    pub(super) fn step_pane(&mut self, forward: bool) -> bool {
        let Some(pane) = neighbour(self.focus, forward) else {
            return false;
        };
        self.focus = pane;
        true
    }
}
