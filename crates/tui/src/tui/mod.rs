//! Terminal user interface, styled like praline's.

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::mpsc;

use crate::prelude::*;

/// What the UI shows, and how keys change it.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

/// Puts the terminal back however [`run`] ends: quit, error, or another
/// component stopping first and dropping it mid-loop.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

/// Draws, waits for a terminal event, applies it, until the user quits.
///
/// # Errors
///
/// Returns an error if the terminal can't be set up, drawn to, or read from.
pub(super) async fn run() -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let _restore = Restore;
    let mut events = read_events();
    let mut state = state::State::default();
    while state.running {
        terminal.draw(|frame| view::render(&mut state, frame))?;
        match events.recv().await {
            Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => state.on_key(key),
            // Anything else (resize, focus, ...) just redraws.
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(error.into()),
            None => break,
        }
    }
    Ok(())
}

/// Reads terminal events on a thread of their own, since `event::read`
/// blocks; the thread ends on the first error or once nobody listens.
fn read_events() -> mpsc::UnboundedReceiver<std::io::Result<Event>> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        loop {
            let event = event::read();
            let failed = event.is_err();
            if tx.send(event).is_err() || failed {
                break;
            }
        }
    });
    rx
}
