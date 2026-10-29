//! Terminal user interface, styled like praline's.

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use todo_client::Client;
use tokio::sync::mpsc;

use crate::prelude::*;

/// What the UI shows, and how keys change it.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

use state::{Action, State};

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
pub(super) async fn run(client: &Client) -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let _restore = Restore;
    let mut events = read_events();
    let mut state = State::default();
    apply(client, &mut state, Action::Refresh).await;
    while state.running {
        terminal.draw(|frame| view::render(&mut state, frame))?;
        match events.recv().await {
            Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                if let Some(action) = state.on_key(key) {
                    // Drawn first so the screen doesn't look stuck while loading.
                    terminal.draw(|frame| view::render(&mut state, frame))?;
                    apply(client, &mut state, action).await;
                }
            }
            // Anything else (resize, focus, ...) just redraws.
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(error.into()),
            None => break,
        }
    }
    Ok(())
}

/// Runs `action` against the server. A failure is shown, not returned,
/// so a server that's down doesn't close the app.
async fn apply(client: &Client, state: &mut State, action: Action) {
    let result = async {
        if action == Action::Refresh {
            state.set_notes(client.notes().await?);
        }
        let todos = match state.note() {
            Some(note) => client.list_note(note).await?,
            None => Vec::new(),
        };
        state.set_todos(todos);
        Ok::<_, todo_client::Error>(())
    }
    .await;
    state.error = result.err().map(|error| error.to_string());
    if let Some(error) = &state.error {
        tracing::warn!("{action:?} failed: {error}");
    }
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
