use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::{app::App, config::Config, prelude::*};

/// Application state handler.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

/// UI types the rest of the module uses often.
mod prelude {
    pub(super) use super::state::*;
    pub(super) use super::theme::*;
}

/// Runs the UI until the user quits, restoring the terminal either way.
pub(crate) fn run(config: Config, app: App) -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, state::State::new(config, app)?);
    ratatui::restore();
    result
}

/// Draws, waits for a key press, applies it, until the state says stop.
fn event_loop(terminal: &mut DefaultTerminal, mut state: state::State) -> Result<()> {
    while state.running {
        terminal.draw(|frame| view::render(&mut state, frame))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            state.on_key(key);
        }
    }
    Ok(())
}
