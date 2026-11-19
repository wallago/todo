//! Local copy of the server's todos, plus the changes not sent yet, so
//! the TUI works offline. Saved to a file after every change.

use std::{
    collections::{BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use todo_client::{Changes, Id, NewTodo, Status, Todo, TodoPatch};

/// A change made here, waiting to reach the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Op {
    /// `POST /api/todos`, always with the id picked here.
    Add(NewTodo),
    /// `PATCH /api/todos/{id}`.
    Update(Id, TodoPatch),
    /// `DELETE /api/todos/{id}`.
    Delete(Id),
}

impl Op {
    /// The todo it changes.
    fn id(&self) -> Option<&Id> {
        match self {
            Self::Add(new) => new.id.as_ref(),
            Self::Update(id, _) | Self::Delete(id) => Some(id),
        }
    }

    /// Short description, for a refused change.
    pub(super) fn describe(&self) -> String {
        match self {
            Self::Add(new) => format!("add \"{}\"", new.text),
            Self::Update(id, _) => format!("edit {}", id.0),
            Self::Delete(id) => format!("delete {}", id.0),
        }
    }
}

/// One queued change, numbered so it can be acknowledged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Pending {
    /// Increasing number, unique within this replica.
    pub(super) seq: u64,
    /// What to send.
    pub(super) op: Op,
}

/// One todo and the note it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    /// Note name, like `projects/todo/main`.
    note: String,
    /// The todo as last known here.
    todo: Todo,
}

/// The local copy.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Replica {
    /// Server it copies; another one starts a fresh copy.
    server: String,
    /// `cursor` of the last sync.
    cursor: u64,
    /// Set when the server refused a change: the next sync fetches
    /// everything instead of what changed since `cursor`.
    resync: bool,
    /// Every note name, sorted.
    notes: BTreeSet<String>,
    /// Every todo, in the order they arrived.
    todos: Vec<Entry>,
    /// Changes not sent yet, oldest first.
    outbox: VecDeque<Pending>,
    /// Number of the next queued change.
    next_seq: u64,
    /// File it's saved to; none in tests.
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Replica {
    /// An empty copy of `server`, saved nowhere.
    pub(super) fn new(server: &str) -> Self {
        Self {
            server: server.to_owned(),
            ..Self::default()
        }
    }

    /// Reads the copy of `server` saved at `path`, or starts an empty one
    /// if there is none, it's unreadable, or it copies another server.
    pub(super) fn load(path: &Path, server: &str) -> Self {
        let saved = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .inspect_err(|error| tracing::warn!("{}: {error}", path.display()))
                .ok(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                tracing::warn!("{}: {error}", path.display());
                None
            }
        };
        let mut replica = saved
            .filter(|replica| replica.server == server)
            .unwrap_or_else(|| Self::new(server));
        replica.path = Some(path.to_owned());
        replica
    }

    /// Writes it to its file, if it has one; a failure is only logged,
    /// since the copy in memory still works.
    pub(super) fn save(&self) {
        let Some(path) = &self.path else { return };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Written aside then renamed, so a crash never leaves half a file.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec(self)?)?;
            std::fs::rename(&tmp, path)
        };
        if let Err(error) = write() {
            tracing::warn!("{}: {error}", path.display());
        }
    }

    /// Every note name, sorted.
    pub(super) fn notes(&self) -> Vec<String> {
        self.notes.iter().cloned().collect()
    }

    /// Every todo.
    pub(super) fn all(&self) -> Vec<Todo> {
        self.todos.iter().map(|entry| entry.todo.clone()).collect()
    }

    /// The todos of `note`.
    pub(super) fn todos(&self, note: &str) -> Vec<Todo> {
        self.todos
            .iter()
            .filter(|entry| entry.note == note)
            .map(|entry| entry.todo.clone())
            .collect()
    }

    /// How many changes wait to be sent.
    pub(super) fn pending(&self) -> usize {
        self.outbox.len()
    }

    /// Adds `new` to its note with a fresh id, then gives it `status`,
    /// and queues both. Returns the todo.
    pub(super) fn add(&mut self, mut new: NewTodo, status: Status, today: NaiveDate) -> Todo {
        let id = new
            .id
            .get_or_insert_with(|| Id(uuid::Uuid::now_v7().to_string()))
            .clone();
        let mut todo = new.to_todo(today);
        // New todos start unticked; any other status is an edit after.
        let patch = (status != Status::Todo).then(|| TodoPatch {
            status: Some(status),
            ..TodoPatch::default()
        });
        if let Some(patch) = &patch {
            patch.clone().apply(&mut todo, today);
        }
        self.notes.insert(new.note.clone());
        self.todos.push(Entry {
            note: new.note.clone(),
            todo: todo.clone(),
        });
        self.queue(Op::Add(new));
        if let Some(patch) = patch {
            self.queue(Op::Update(id, patch));
        }
        todo
    }

    /// Applies `patch` to todo `id` and queues it. Returns the todo, or
    /// `None` if there's no such todo.
    pub(super) fn update(&mut self, id: &Id, patch: TodoPatch, today: NaiveDate) -> Option<Todo> {
        let entry = self.todos.iter_mut().find(|entry| &entry.todo.id == id)?;
        patch.clone().apply(&mut entry.todo, today);
        let todo = entry.todo.clone();
        self.queue(Op::Update(id.clone(), patch));
        Some(todo)
    }

    /// Removes todo `id` and queues it.
    pub(super) fn delete(&mut self, id: &Id) {
        self.todos.retain(|entry| &entry.todo.id != id);
        self.queue(Op::Delete(id.clone()));
    }

    /// The oldest change not sent yet.
    pub(super) fn next(&self) -> Option<Pending> {
        self.outbox.front().cloned()
    }

    /// Change `seq` reached the server: drops it from the queue.
    pub(super) fn sent(&mut self, seq: u64) {
        self.outbox.retain(|pending| pending.seq != seq);
    }

    /// The server refused change `seq`: drops it, and has the next sync
    /// fetch everything, to undo it here too.
    pub(super) fn refused(&mut self, seq: u64) {
        self.sent(seq);
        self.resync = true;
    }

    /// What the next sync should ask for: `since`, and whether that's
    /// everything.
    pub(super) fn since(&self) -> u64 {
        if self.resync { 0 } else { self.cursor }
    }

    /// Takes in what changed on the server since [`Self::since`]. Todos
    /// with a change still queued keep their local version: the server
    /// sends them again once it has that change.
    pub(super) fn merge(&mut self, changes: Changes, since: u64) {
        if since == 0 {
            // Everything: keep only what's still waiting to be sent.
            self.notes.clear();
            let waiting: BTreeSet<Id> = self
                .outbox
                .iter()
                .filter_map(|p| p.op.id())
                .cloned()
                .collect();
            self.todos.retain(|entry| waiting.contains(&entry.todo.id));
            self.notes
                .extend(self.todos.iter().map(|entry| entry.note.clone()));
            self.resync = false;
        }
        for note in changes.notes {
            if note.deleted {
                self.notes.remove(&note.name);
            } else {
                self.notes.insert(note.name);
            }
        }
        for change in changes.todos {
            let id = &change.todo.id;
            if self
                .outbox
                .iter()
                .any(|pending| pending.op.id() == Some(id))
            {
                continue;
            }
            let at = self.todos.iter().position(|entry| &entry.todo.id == id);
            match (at, change.deleted) {
                (Some(at), true) => {
                    self.todos.remove(at);
                }
                (None, true) => {}
                (Some(at), false) => {
                    if let Some(entry) = self.todos.get_mut(at) {
                        entry.note = change.note;
                        entry.todo = change.todo;
                    }
                }
                (None, false) => self.todos.push(Entry {
                    note: change.note,
                    todo: change.todo,
                }),
            }
        }
        self.cursor = changes.cursor;
    }

    /// Queues `op`.
    fn queue(&mut self, op: Op) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.outbox.push_back(Pending { seq, op });
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use todo_client::{NoteChange, TodoChange};

    use super::*;

    const TODAY: NaiveDate = NaiveDate::MIN;

    fn new(text: &str) -> NewTodo {
        NewTodo {
            text: text.into(),
            note: "errands".into(),
            ..NewTodo::default()
        }
    }

    fn change(note: &str, todo: Todo, deleted: bool) -> TodoChange {
        TodoChange {
            note: note.into(),
            deleted,
            todo,
        }
    }

    fn changes(cursor: u64, todos: Vec<TodoChange>) -> Changes {
        Changes {
            cursor,
            notes: Vec::new(),
            todos,
        }
    }

    #[test]
    fn add_shows_at_once_and_queues_with_its_id() {
        let mut replica = Replica::new("http://server");
        let todo = replica.add(new("milk"), Status::Todo, TODAY);
        assert_eq!(todo.id.0.len(), 36, "a UUID");
        assert_eq!(replica.todos("errands"), vec![todo.clone()]);
        assert_eq!(replica.notes(), vec!["errands".to_owned()]);
        let Some(Pending {
            op: Op::Add(sent), ..
        }) = replica.next()
        else {
            panic!("an add is queued");
        };
        assert_eq!(sent.id, Some(todo.id));
    }

    #[test]
    fn add_with_a_status_queues_an_edit_after() {
        let mut replica = Replica::new("s");
        let todo = replica.add(new("milk"), Status::Done, TODAY);
        assert_eq!(todo.status, Status::Done);
        assert_eq!(todo.done, Some(TODAY));
        assert_eq!(replica.pending(), 2);
    }

    #[test]
    fn edits_and_deletes_apply_here_then_queue() {
        let mut replica = Replica::new("s");
        let id = replica.add(new("milk"), Status::Todo, TODAY).id;
        let patch = TodoPatch {
            text: Some("oat milk".into()),
            ..TodoPatch::default()
        };
        let edited = replica.update(&id, patch, TODAY);
        assert_eq!(edited.map(|todo| todo.text), Some("oat milk".into()));
        replica.delete(&id);
        assert_eq!(replica.todos("errands"), Vec::new());
        assert_eq!(replica.pending(), 3);
        assert_eq!(
            replica.update(&Id("gone".into()), TodoPatch::default(), TODAY),
            None
        );
    }

    #[test]
    fn sent_changes_leave_the_queue() {
        let mut replica = Replica::new("s");
        replica.add(new("milk"), Status::Todo, TODAY);
        replica.add(new("eggs"), Status::Todo, TODAY);
        let first = replica.next().map(|pending| pending.seq);
        replica.sent(first.unwrap_or_default());
        assert_eq!(replica.pending(), 1);
        assert_ne!(replica.next().map(|pending| pending.seq), first);
    }

    #[test]
    fn merge_adds_changes_and_removes_deleted() {
        let mut replica = Replica::new("s");
        let a = Todo::new(Id("a".into()), "milk", TODAY);
        let b = Todo::new(Id("b".into()), "eggs", TODAY);
        let mut update = changes(
            2,
            vec![
                change("errands", a.clone(), false),
                change("home", b, false),
            ],
        );
        update.notes = vec![
            NoteChange {
                name: "errands".into(),
                deleted: false,
            },
            NoteChange {
                name: "home".into(),
                deleted: false,
            },
        ];
        replica.merge(update, 0);
        assert_eq!(replica.since(), 2);
        assert_eq!(replica.all().len(), 2);
        let mut gone = changes(
            3,
            vec![change(
                "home",
                Todo::new(Id("b".into()), "eggs", TODAY),
                true,
            )],
        );
        gone.notes = vec![NoteChange {
            name: "home".into(),
            deleted: true,
        }];
        replica.merge(gone, 2);
        assert_eq!(replica.all(), vec![a]);
        assert_eq!(replica.notes(), vec!["errands".to_owned()]);
    }

    #[test]
    fn merge_keeps_todos_with_changes_still_queued() {
        let mut replica = Replica::new("s");
        let id = replica.add(new("milk"), Status::Todo, TODAY).id;
        let stale = Todo::new(id.clone(), "old text", TODAY);
        replica.merge(changes(5, vec![change("errands", stale, false)]), 0);
        let texts: Vec<String> = replica.all().into_iter().map(|todo| todo.text).collect();
        assert_eq!(texts, vec!["milk".to_owned()]);
        assert_eq!(replica.notes(), vec!["errands".to_owned()]);
    }

    #[test]
    fn a_refused_change_is_undone_by_the_next_full_sync() {
        let mut replica = Replica::new("s");
        replica.merge(changes(4, Vec::new()), 0);
        replica.add(new("milk"), Status::Todo, TODAY);
        let seq = replica
            .next()
            .map(|pending| pending.seq)
            .unwrap_or_default();
        replica.refused(seq);
        assert_eq!(replica.since(), 0, "everything");
        replica.merge(changes(4, Vec::new()), 0);
        assert_eq!(replica.all(), Vec::new());
        assert_eq!(replica.notes(), Vec::<String>::new());
        assert_eq!(replica.since(), 4);
    }

    #[test]
    fn saves_and_loads_its_file_for_the_same_server_only() {
        let dir = std::env::temp_dir().join(format!("todo-tui-{}", uuid::Uuid::now_v7()));
        let path = dir.join("replica.json");
        let mut replica = Replica::load(&path, "http://a");
        replica.add(new("milk"), Status::Todo, TODAY);
        replica.save();
        let again = Replica::load(&path, "http://a");
        assert_eq!(again.all(), replica.all());
        assert_eq!(again.pending(), 1);
        assert_eq!(Replica::load(&path, "http://b").all(), Vec::new());
        let _ = std::fs::remove_dir_all(dir);
    }
}
