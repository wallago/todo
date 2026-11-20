//! Terminal user interface, styled like praline's.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError},
    time::Duration,
};

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use todo_client::{Client, Status, Todo, TodoPatch};
use tokio::{
    sync::{
        Mutex,
        mpsc::{self, UnboundedSender},
    },
    time::{self, Instant, MissedTickBehavior},
};

use crate::{config::Keys, prelude::*};

/// Local copy of the todos and the changes not sent yet.
mod replica;
/// What the UI shows, and how keys change it.
mod state;
/// Colour palette shared by every widget.
mod theme;
/// Widget renderer.
mod view;

use replica::{Op, Replica};
use state::{Action, Loaded, Page, State, SyncStatus, TopicStats};

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
/// How often the topics and the open page reload on their own.
const AUTO_REFRESH: Duration = Duration::from_secs(30);

/// What a sync sends back to the event loop: its id and what it loaded.
type Done = (u64, Vec<Loaded>);

/// Draws, then waits for a key, a finished sync or a spinner tick, until
/// the user quits. Reads and changes go to the local [`Replica`] at once;
/// syncs with the server run on their own task, so the UI never waits.
///
/// # Errors
///
/// Returns an error if the terminal can't be set up, drawn to, or read from.
pub(super) async fn run(client: &Client, server: &str, keys: Keys) -> Result<()> {
    let shared = Arc::new(Shared {
        replica: StdMutex::new(Replica::load(&replica_path(), server)),
        syncing: Mutex::new(()),
    });
    let mut terminal = ratatui::try_init()?;
    let _restore = Restore;
    let mut events = read_events();
    let (done_tx, mut done) = mpsc::unbounded_channel();
    let mut ticks = time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut refresh = time::interval_at(Instant::now() + AUTO_REFRESH, AUTO_REFRESH);
    refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut state = State {
        keys,
        ..State::default()
    };
    let link = Link {
        client,
        shared: &shared,
        done: &done_tx,
    };
    link.spawn(&mut state, Action::Refresh, false);
    while state.running {
        terminal.draw(|frame| view::render(&mut state, frame))?;
        tokio::select! {
            event = events.recv() => match event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if let Some(action) = state.on_key(key) {
                        link.spawn(&mut state, action, false);
                    }
                }
                // Anything else (resize, focus, ...) just redraws.
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            Some((id, loaded)) = done.recv() => state.finish(id, loaded),
            _ = ticks.tick(), if state.is_busy() => state.tick = state.tick.wrapping_add(1),
            // Quiet, and skipped while another sync runs, so a slow server
            // doesn't pile them up.
            _ = refresh.tick(), if state.is_idle() => link.spawn(&mut state, Action::Refresh, true),
        }
    }
    Ok(())
}

/// `<data dir>/todo-tui/replica.json`.
fn replica_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(env!("CARGO_PKG_NAME"))
        .join("replica.json")
}

/// The replica, shared with the sync task.
#[derive(Debug)]
struct Shared {
    /// The local copy. Held only briefly, never across a server call.
    replica: StdMutex<Replica>,
    /// Held for a whole sync, so two never send the same change at once.
    syncing: Mutex<()>,
}

impl Shared {
    /// The replica, even if a task panicked holding it.
    fn lock(&self) -> MutexGuard<'_, Replica> {
        self.replica.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What [`Link::spawn`] needs, borrowed from [`run`].
struct Link<'a> {
    /// The server.
    client: &'a Client,
    /// The local copy.
    shared: &'a Arc<Shared>,
    /// Where syncs send what they bring back.
    done: &'a UnboundedSender<Done>,
}

impl Link<'_> {
    /// Runs `action`: on the replica right away, then, for a refresh or a
    /// change, a sync with the server on its own task. A `quiet` refresh
    /// shows no spinner.
    fn spawn(&self, state: &mut State, action: Action, quiet: bool) {
        let job = Job {
            topic: state.manager.topic.clone(),
            topics: state.topics.clone(),
            selected: state.manager.selected().cloned(),
            stats: state.page == Page::Stats || state.picker.is_some(),
            dashboard: state.page == Page::Dashboard,
        };
        let sync = matches!(
            action,
            Action::Refresh
                | Action::Toggle
                | Action::Add(..)
                | Action::Change(..)
                | Action::Delete(_)
        );
        let id = state.start(&action, true);
        let loaded = {
            let mut replica = self.shared.lock();
            let loaded = local(&mut replica, &job, action);
            replica.save();
            loaded
        };
        state.finish(id, loaded);
        if !sync {
            return;
        }
        // Any sync on screen turns the title spinner until it's back.
        state.syncing |= !quiet;
        let id = state.start(&Action::Refresh, true);
        let client = self.client.clone();
        let shared = Arc::clone(self.shared);
        let done = self.done.clone();
        tokio::spawn(async move {
            let status = sync_with(&client, &shared).await;
            let mut loaded = vec![Loaded::Sync(status)];
            loaded.extend(local(&mut shared.lock(), &job, Action::Refresh));
            // Only fails once the app is closing.
            let _ = done.send((id, loaded));
        });
    }
}

/// What an action needs from the state, copied when it starts.
struct Job {
    /// The manager's topic.
    topic: Option<String>,
    /// Every topic path.
    topics: Vec<String>,
    /// The todo under the cursor.
    selected: Option<Todo>,
    /// Whether the stats page or the topic picker is open.
    stats: bool,
    /// Whether the dashboard is open.
    dashboard: bool,
}

/// Runs `action` on the replica, returning what changed on screen.
fn local(replica: &mut Replica, job: &Job, action: Action) -> Vec<Loaded> {
    let today = chrono::Local::now().date_naive();
    let mut loaded = Vec::new();
    match action {
        Action::Refresh => {
            let topics = replica.topics();
            // With no topic open, the manager opens the first one.
            let topic = job.topic.clone().or_else(|| topics.first().cloned());
            if job.stats {
                loaded.push(load_stats(replica, &topics));
            }
            if job.dashboard {
                loaded.push(load_dashboard(replica, &topics));
            }
            loaded.push(Loaded::Topics(topics));
            loaded.push(Loaded::Tags(replica.all()));
            loaded.extend(load_todos(replica, topic));
        }
        Action::LoadTodos => loaded.extend(load_todos(replica, job.topic.clone())),
        Action::LoadStats => loaded.push(load_stats(replica, &job.topics)),
        Action::LoadDashboard => loaded.push(load_dashboard(replica, &job.topics)),
        Action::Jump(id) => {
            loaded.extend(load_todos(replica, job.topic.clone()));
            loaded.push(Loaded::Select(id));
        }
        Action::Toggle => {
            if let Some(todo) = &job.selected {
                // Ticking a recurring todo adds its next one on the server,
                // which comes back with the sync that follows.
                let status = if todo.status.is_closed() {
                    Status::Todo
                } else {
                    Status::Done
                };
                let patch = TodoPatch {
                    status: Some(status),
                    ..TodoPatch::default()
                };
                loaded.extend(replica.update(&todo.id, patch, today).map(Loaded::Changed));
            }
        }
        Action::Add(mut new, status) => {
            // `a` only opens the form with a topic open.
            new.topic = job.topic.clone().unwrap_or_default();
            let added = replica.add(new, status, today);
            // A first todo creates its topic, so the list may have grown.
            loaded.push(Loaded::Topics(replica.topics()));
            loaded.extend(load_todos(replica, job.topic.clone()));
            loaded.push(Loaded::Select(added.id));
        }
        Action::Change(id, patch) => {
            loaded.extend(replica.update(&id, patch, today).map(Loaded::Changed));
        }
        Action::Delete(id) => {
            replica.delete(&id);
            loaded.extend(load_todos(replica, job.topic.clone()));
        }
    }
    loaded.push(Loaded::Pending(replica.pending()));
    loaded
}

/// `topic`'s todos, if a topic is open.
fn load_todos(replica: &Replica, topic: Option<String>) -> Option<Loaded> {
    topic.map(|topic| {
        let todos = replica.todos(&topic);
        Loaded::Todos(topic, todos)
    })
}

/// Every topic's counts and next due date.
fn load_stats(replica: &Replica, topics: &[String]) -> Loaded {
    let rows = topics
        .iter()
        .map(|topic| {
            let todos = replica.todos(topic);
            let done = todos.iter().filter(|todo| todo.status.is_closed()).count();
            let next_due = todos
                .iter()
                .filter(|todo| !todo.status.is_closed())
                .filter_map(|todo| todo.due)
                .min();
            TopicStats {
                topic: topic.clone(),
                done,
                total: todos.len(),
                next_due,
            }
        })
        .collect();
    Loaded::Stats(rows)
}

/// Every topic's todos, for the dashboard.
fn load_dashboard(replica: &Replica, topics: &[String]) -> Loaded {
    let all = topics
        .iter()
        .map(|topic| (topic.clone(), replica.todos(topic)))
        .collect();
    Loaded::Dashboard(all)
}

/// Sends the queued changes, oldest first, then takes in what changed on
/// the server. Stops at the first change the server can't take right now
/// (down, timeout, bad token), keeping it queued for next time; a change
/// it refuses outright is dropped and undone here.
async fn sync_with(client: &Client, shared: &Shared) -> SyncStatus {
    let _one = shared.syncing.lock().await;
    let mut status = SyncStatus::default();
    if let Err(error) = push_pull(client, shared, &mut status).await {
        tracing::warn!("sync failed: {}", with_causes(&error));
        status.problem = Some(match error {
            todo_client::Error::Http(_) => "offline".to_owned(),
            todo_client::Error::Api { .. } => error.to_string(),
        });
    }
    let replica = shared.lock();
    replica.save();
    status.pending = replica.pending();
    status
}

/// The steps of [`sync_with`], stopping at the first error.
async fn push_pull(
    client: &Client,
    shared: &Shared,
    status: &mut SyncStatus,
) -> todo_client::Result<()> {
    loop {
        let next = shared.lock().next();
        let Some(pending) = next else { break };
        match send(client, &pending.op).await {
            Ok(()) => shared.lock().sent(pending.seq),
            Err(error) if error.is_refused() => {
                let why = format!("{}: {error}", pending.op.describe());
                tracing::warn!("refused: {why}");
                status.refused = Some(why);
                shared.lock().refused(pending.seq);
            }
            Err(error) => return Err(error),
        }
    }
    let mut since = shared.lock().since();
    let mut changes = client.sync(since).await?;
    // A cursor going back means a new database: start over.
    if changes.cursor < since {
        since = 0;
        changes = client.sync(0).await?;
    }
    shared.lock().merge(changes, since);
    Ok(())
}

/// Sends one queued change. Deleting a todo already gone is fine.
async fn send(client: &Client, op: &Op) -> todo_client::Result<()> {
    match op {
        Op::Add(new) => client.add(new).await.map(drop),
        Op::Update(id, patch) => client.update(id, patch).await.map(drop),
        Op::Delete(id) => match client.remove(id).await {
            Err(error) if error.is_not_found() => Ok(()),
            result => result,
        },
    }
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
