//! Terminal user interface, styled like praline's.

use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use todo_client::{Client, Status, Todo, TodoPatch};
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    time::{self, MissedTickBehavior},
};

use crate::{config::Keys, prelude::*};

/// What the UI shows, and how keys change it.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

use state::{Action, Loaded, NoteStats, Page, State};

/// Puts the terminal back however [`run`] ends: quit, error, or another
/// component stopping first and dropping it mid-loop.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

/// How often the spinner turns while something loads.
const TICK: Duration = Duration::from_millis(100);

/// What a server call sends back to the event loop: its id and result.
type Done = (u64, std::result::Result<Vec<Loaded>, String>);

/// Draws, then waits for a key, a finished server call or a spinner tick,
/// until the user quits. Server calls run on their own task, so the UI
/// never waits for them.
///
/// # Errors
///
/// Returns an error if the terminal can't be set up, drawn to, or read from.
pub(super) async fn run(client: &Client, keys: Keys) -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let _restore = Restore;
    let mut events = read_events();
    let (done_tx, mut done) = mpsc::unbounded_channel();
    let mut ticks = time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut state = State {
        keys,
        ..State::default()
    };
    spawn(client, &mut state, Action::Refresh, &done_tx);
    while state.running {
        terminal.draw(|frame| view::render(&mut state, frame))?;
        tokio::select! {
            event = events.recv() => match event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if let Some(action) = state.on_key(key) {
                        spawn(client, &mut state, action, &done_tx);
                    }
                }
                // Anything else (resize, focus, ...) just redraws.
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            Some((id, result)) = done.recv() => state.finish(id, result),
            _ = ticks.tick(), if state.is_busy() => state.tick = state.tick.wrapping_add(1),
        }
    }
    Ok(())
}

/// Runs `action` against the server on its own task, the result coming
/// back through `done`. A failure is shown, not returned, so a server
/// that's down doesn't close the app.
fn spawn(client: &Client, state: &mut State, action: Action, done: &UnboundedSender<Done>) {
    let id = state.start(&action);
    let job = Job {
        note: state.manager.note.clone(),
        notes: state.notes.clone(),
        selected: state.manager.selected().cloned(),
        stats: state.page == Page::Stats,
    };
    let client = client.clone();
    let done = done.clone();
    tokio::spawn(async move {
        let what = format!("{action:?}");
        let result = load(&client, job, action).await.map_err(|error| {
            let error = with_causes(&error);
            tracing::warn!("{what} failed: {error}");
            error
        });
        // Only fails once the app is closing.
        let _ = done.send((id, result));
    });
}

/// What a server call needs from the state, copied when it starts.
struct Job {
    /// The manager's note.
    note: Option<String>,
    /// Every note path.
    notes: Vec<String>,
    /// The todo under the cursor.
    selected: Option<Todo>,
    /// Whether the stats page is open.
    stats: bool,
}

/// The server calls behind [`spawn`].
async fn load(client: &Client, job: Job, action: Action) -> todo_client::Result<Vec<Loaded>> {
    let mut loaded = Vec::new();
    match action {
        Action::Refresh => {
            let notes = client.notes().await?;
            // With no note open, the manager opens the first one.
            let note = job.note.or_else(|| notes.first().cloned());
            if job.stats {
                loaded.push(load_stats(client, &notes).await?);
            }
            loaded.push(Loaded::Notes(notes));
            loaded.extend(load_todos(client, note).await?);
        }
        Action::LoadTodos => loaded.extend(load_todos(client, job.note).await?),
        Action::LoadStats => loaded.push(load_stats(client, &job.notes).await?),
        Action::Toggle => {
            if let Some(todo) = job.selected {
                let toggled = if todo.status.is_closed() {
                    let reopen = TodoPatch {
                        status: Some(Status::Todo),
                        ..TodoPatch::default()
                    };
                    client.update(&todo.id, &reopen).await?
                } else {
                    client.complete(&todo.id).await?
                };
                loaded.push(Loaded::Changed(toggled));
            }
        }
        Action::Add(mut new, status) => {
            new.note.clone_from(&job.note);
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
            loaded.push(Loaded::Notes(client.notes().await?));
            loaded.extend(load_todos(client, job.note).await?);
            loaded.push(Loaded::Select(added.id));
        }
        Action::Change(id, patch) => {
            loaded.push(Loaded::Changed(client.update(&id, &patch).await?));
        }
        Action::Delete(id) => {
            client.remove(&id).await?;
            loaded.extend(load_todos(client, job.note).await?);
        }
    }
    Ok(loaded)
}

/// Reloads `note`'s todos, if a note is open.
async fn load_todos(client: &Client, note: Option<String>) -> todo_client::Result<Option<Loaded>> {
    Ok(match note {
        Some(note) => {
            let todos = client.list_note(&note).await?;
            Some(Loaded::Todos(note, todos))
        }
        None => None,
    })
}

/// Reloads every note's counts.
async fn load_stats(client: &Client, notes: &[String]) -> todo_client::Result<Loaded> {
    let mut rows = Vec::with_capacity(notes.len());
    for note in notes {
        let todos = client.list_note(note).await?;
        let done = todos.iter().filter(|todo| todo.status.is_closed()).count();
        rows.push(NoteStats {
            note: note.clone(),
            done,
            total: todos.len(),
        });
    }
    Ok(Loaded::Stats(rows))
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
