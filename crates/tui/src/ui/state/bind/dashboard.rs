use std::{env::current_dir, path::Path};

use ratatui::style::Style;
use ratatui_explorer::{FileExplorer, FileExplorerBuilder, Theme};
use strum::IntoEnumIterator;
use tui_input::Input;

use super::prelude::*;
use crate::{
    app::{App, prelude::OptId},
    prelude::*,
    ui::{prelude::*, state::bind::edge},
};

impl Dashboard {
    /// Runs `action` against the focused pane.
    pub(super) fn on_action(&mut self, action: Action, app: &mut App) -> Result<()> {
        match (self.focus, action) {
            (DashboardPane::Options, Action::Up | Action::Down) => {
                step_in(
                    &mut self.options,
                    OptId::iter().len(),
                    action == Action::Down,
                );
            }
            (DashboardPane::Options, Action::Enter) => {
                let Some((id, conflicted)) = self
                    .options
                    .selected()
                    .and_then(|at| app.options.get(at))
                    .map(|opt| (opt.id, opt.conflicted))
                else {
                    return Ok(());
                };
                if conflicted {
                    return Ok(());
                }
                app.nested_check(id, None);
                app.mark_conflicts();
                app.generate()?;
            }
            (DashboardPane::Form, Action::Enter) => {
                if let Some(field) = self.row.field() {
                    self.editing = Some(Input::new(field.get(app).to_owned()));
                } else if self.row == FormRow::Dest {
                    self.picker = dir_picker(&app.dest).ok();
                }
            }
            (DashboardPane::Form, Action::Up | Action::Down) => {
                self.row = neighbour(self.row, action == Action::Down)
                    .unwrap_or_else(|| edge(action == Action::Down));
            }
            _ => {}
        }
        Ok(())
    }

    /// Drives the open picker: move, step in and out of directories,
    /// `confirm` takes the one you're in, `leave` closes it untouched.
    pub(super) fn on_picker_action(&mut self, action: Action, app: &mut App) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let _ = match action {
            Action::Up => picker.handle(ratatui_explorer::Input::Up),
            Action::Down => picker.handle(ratatui_explorer::Input::Down),
            Action::Parent => picker.handle(ratatui_explorer::Input::Left),
            Action::Start => current_dir().and_then(|dir| picker.set_cwd(dir)),
            Action::Right => picker.set_cwd(picker.current().path.clone()),
            Action::Validate => {
                app.dest.clone_from(picker.cwd());
                self.picker = None;
                Ok(())
            }
            Action::Enter => picker.handle(ratatui_explorer::Input::Right),
            Action::Leave => {
                self.picker = None;
                Ok(())
            }
            _ => Ok(()),
        };
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

/// A picker over the directories under `dir`. `Theme::new`, not `default`:
/// the default brings its own border, and the modal draws one already.
fn dir_picker(dir: &Path) -> std::io::Result<FileExplorer> {
    let theme = Theme::new()
        .with_dir_style(SECONDARY)
        .with_highlight_symbol("> ")
        .with_highlight_item_style(Style::default().fg(PRIMARY).bold())
        .with_highlight_dir_style(Style::default().fg(HIGHLIGHT).bold());
    FileExplorerBuilder::default()
        .working_dir(dir)
        .filter_map(|file| file.is_dir.then_some(file))
        .theme(theme)
        .build()
}
