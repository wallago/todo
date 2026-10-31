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
    let what = format!("{action:?}");
    let result = load(client, state, action).await;
    state.error = result.err().map(|error| with_causes(&error));
    if let Some(error) = &state.error {
        tracing::warn!("{what} failed: {error}");
    }
}

/// The server calls behind [`apply`].
async fn load(client: &Client, state: &mut State, action: Action) -> todo_client::Result<()> {
    match action {
        Action::Refresh => {
            state.set_notes(client.notes().await?);
            state.set_tags(&client.list().await?);
            if state.page == Page::Stats {
                load_stats(client, state).await?;
            }
            load_todos(client, state).await
        }
        Action::LoadTodos => load_todos(client, state).await,
        Action::LoadStats => load_stats(client, state).await,
        Action::Toggle => {
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
            Ok(())
        }
        Action::Add(mut new, status) => {
            new.note.clone_from(&state.manager.note);
            let added = client.add(&new).await?;
            // New todos start unticked; set any other status after.
            if status != Status::Todo {
                let patch = TodoPatch {
                    status: Some(status),
                    ..TodoPatch::default()
                };
                client.update(&added.id, &patch).await?;
            }
            // A first todo creates its note, so the list may have grown.
            state.set_notes(client.notes().await?);
            load_todos(client, state).await?;
            state.manager.select(&added.id);
            Ok(())
        }
        Action::Change(id, patch) => {
            let changed = client.update(&id, &patch).await?;
            state.learn_tags(std::slice::from_ref(&changed));
            state.manager.replace(changed);
            Ok(())
        }
        Action::Delete(id) => {
            client.remove(&id).await?;
            load_todos(client, state).await
        }
    }
}

/// Reloads the manager's note.
async fn load_todos(client: &Client, state: &mut State) -> todo_client::Result<()> {
    let todos = match &state.manager.note {
        Some(note) => client.list_note(note).await?,
        None => Vec::new(),
    };
    state.learn_tags(&todos);
    state.manager.set_todos(todos);
    Ok(())
}

/// Reloads every note's counts.
async fn load_stats(client: &Client, state: &mut State) -> todo_client::Result<()> {
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
