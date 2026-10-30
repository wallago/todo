//! Terminal user interface, styled like praline's.

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use todo_client::{Client, Status, TodoPatch};
use tokio::sync::mpsc;

use crate::{config::Keys, prelude::*};

/// What the UI shows, and how keys change it.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

use state::{Action, NoteStats, Page, State};

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
pub(super) async fn run(client: &Client, keys: Keys) -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let _restore = Restore;
    let mut events = read_events();
    let mut state = State {
        keys,
        ..State::default()
    };
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
    let result = load(client, state, action).await;
    state.error = result.err().map(|error| with_causes(&error));
    if let Some(error) = &state.error {
        tracing::warn!("{action:?} failed: {error}");
    }
}

/// The server calls behind [`apply`].
async fn load(client: &Client, state: &mut State, action: Action) -> todo_client::Result<()> {
    if action == Action::Refresh {
        state.set_notes(client.notes().await?);
    }
    if action == Action::LoadStats || (action == Action::Refresh && state.page == Page::Stats) {
        let mut rows = Vec::with_capacity(state.notes.len());
        for note in &state.notes {
            let todos = client.list_note(note).await?;
            let done = todos.iter().filter(|todo| todo.status.is_closed()).count();
            rows.push(NoteStats {
                note: note.clone(),
                done,
                total: todos.len(),
            });
        }
        state.stats = rows;
    }
    if action == Action::Toggle {
        if let Some(todo) = state.manager.selected() {
            let toggled = if todo.status.is_closed() {
                let reopen = TodoPatch {
                    status: Some(Status::Todo),
                    ..TodoPatch::default()
                };
                client.update(&todo.id, &reopen).await?
            } else {
                client.complete(&todo.id).await?
            };
            state.manager.replace(toggled);
        }
        return Ok(());
    }
    if action != Action::LoadStats {
        let todos = match &state.manager.note {
            Some(note) => client.list_note(note).await?,
            None => Vec::new(),
        };
        state.manager.set_todos(todos);
    }
    Ok(())
}

/// `error: cause: cause...`, since reqwest hides the useful part in the sources.
fn with_causes(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
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
