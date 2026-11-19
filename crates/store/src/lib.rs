//! Owns the todo database: one SQLite file with notes and todos.
//!
//! A note is a subject (a project, a trip...) named like a path,
//! `projects/todo/main.md`. Deleting a todo only marks it deleted
//! (`deleted_at`), so offline clients can learn about it later.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, SecondsFormat, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::{Serialize, de::DeserializeOwned};
use todo_core::{Id, Recur, Status, Todo};
use uuid::Uuid;

/// What can go wrong in the store.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The database refused or failed.
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    /// Reading a vault file failed (import only).
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        source: io::Error,
    },
    /// No todo has this id.
    #[error("no todo with id `{}`", .0.0)]
    NotFound(Id),
    /// Another todo, maybe a deleted one, already has this id.
    #[error("id `{}` is already used", .0.0)]
    DuplicateId(Id),
    /// The note name is not a relative `.md` path.
    #[error("`{0}` is not a note name: give a relative path to a .md note, like `projects/dev.md`")]
    BadNote(String),
}

/// Shorthand for the store's results.
pub type Result<T> = std::result::Result<T, Error>;

/// Schema changes, in order. `PRAGMA user_version` counts those applied.
/// Only ever append: a deployed database already ran the old ones.
const MIGRATIONS: &[&str] = &[r"
CREATE TABLE notes (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE TABLE todos (
    id         TEXT PRIMARY KEY,
    note_id    TEXT NOT NULL REFERENCES notes(id),
    text       TEXT NOT NULL,
    status     TEXT NOT NULL CHECK (status IN ('todo', 'in_progress', 'done', 'cancelled')),
    priority   TEXT NOT NULL CHECK (priority IN ('lowest', 'low', 'normal', 'medium', 'high', 'highest')),
    due        TEXT,
    done       TEXT,
    tags       TEXT NOT NULL DEFAULT '[]',
    pinned     INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    recur      TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX todos_note_id ON todos(note_id);
CREATE INDEX todos_updated_at ON todos(updated_at);
"];

/// Columns read into a [`Todo`] by [`read_todo`]; column 8 is its note's name.
const TODO_COLUMNS: &str = "t.id, t.text, t.status, t.priority, t.due, t.done, t.tags, \
     t.created_at, n.name, t.pinned, t.recur";

/// The todo database.
#[derive(Debug)]
pub struct Store {
    /// Open connection.
    conn: Connection,
    /// Note that new todos go to when none is given.
    inbox: String,
}

impl Store {
    /// Opens the database file, creating it and its tables if missing.
    /// New todos without a note go to `inbox`.
    ///
    /// # Errors
    ///
    /// [`Error::Db`] if the file can't be opened or migrated,
    /// [`Error::BadNote`] if `inbox` is not a note name.
    pub fn open(path: impl AsRef<Path>, inbox: impl Into<String>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_owned(),
                source,
            })?;
        }
        let conn = Connection::open(path)?;
        // WAL lets `todo-server import` write while the server reads.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::setup(conn, inbox.into())
    }

    /// A database in memory, for tests.
    ///
    /// # Errors
    ///
    /// As [`Store::open`].
    pub fn open_in_memory(inbox: impl Into<String>) -> Result<Self> {
        Self::setup(Connection::open_in_memory()?, inbox.into())
    }

    /// Settings and migrations shared by both `open`s.
    fn setup(mut conn: Connection, inbox: String) -> Result<Self> {
        check_note(&inbox)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;
        Ok(Self { conn, inbox })
    }

    /// Every todo with its note's name, note by note, in the order they were
    /// added. Only those of `note` if given.
    ///
    /// # Errors
    ///
    /// [`Error::Db`] if the query fails.
    pub fn todos_by_note(&self, note: Option<&str>) -> Result<Vec<(String, Todo)>> {
        let sql = format!(
            "SELECT {TODO_COLUMNS} FROM todos t JOIN notes n ON n.id = t.note_id
             WHERE t.deleted_at IS NULL AND (?1 IS NULL OR n.name = ?1)
             ORDER BY n.name, t.rowid"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map([note], |row| Ok((row.get(8)?, read_todo(row)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every note's name, todos or not, sorted.
    ///
    /// # Errors
    ///
    /// [`Error::Db`] if the query fails.
    pub fn notes(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT name FROM notes WHERE deleted_at IS NULL ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The todo with this id, unless deleted.
    ///
    /// # Errors
    ///
    /// [`Error::Db`] if the query fails.
    pub fn get(&self, id: &Id) -> Result<Option<Todo>> {
        get(&self.conn, id)
    }

    /// Adds a todo to the inbox note. Otherwise like [`Store::add_to`].
    ///
    /// # Errors
    ///
    /// As [`Store::add_to`].
    pub fn add(&mut self, todo: Todo) -> Result<Todo> {
        let inbox = self.inbox.clone();
        self.add_to(&inbox, todo)
    }

    /// Adds a todo to `note`, creating the note if it is missing. A todo
    /// without an id gets a new UUID. Returns it as saved.
    ///
    /// # Errors
    ///
    /// [`Error::BadNote`] if `note` is not a note name,
    /// [`Error::DuplicateId`] if its id is taken, [`Error::Db`] on failure.
    pub fn add_to(&mut self, note: &str, mut todo: Todo) -> Result<Todo> {
        check_note(note)?;
        if todo.id.0.is_empty() {
            todo.id = Id(Uuid::now_v7().to_string());
        }
        let now = now();
        let tx = self.conn.transaction()?;
        if id_taken(&tx, &todo.id)? {
            return Err(Error::DuplicateId(todo.id));
        }
        let note_id = ensure_note(&tx, note, &now)?;
        insert(&tx, &note_id, &todo, &now)?;
        let saved = get(&tx, &todo.id)?.ok_or_else(|| Error::NotFound(todo.id.clone()))?;
        tx.commit()?;
        Ok(saved)
    }

    /// Changes a todo. Its id can't change. Returns it as saved.
    ///
    /// Completing a recurring todo also adds the next one to its note (see
    /// [`todo_core::Recur::follow_up`]); the done one stops recurring, so
    /// reopening and completing it again doesn't add a second copy.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no todo has this id, [`Error::Db`] on failure.
    pub fn update(&mut self, id: &Id, change: impl FnOnce(&mut Todo)) -> Result<Todo> {
        let now = now();
        let tx = self.conn.transaction()?;
        let mut todo = get(&tx, id)?.ok_or_else(|| Error::NotFound(id.clone()))?;
        let before = todo.clone();
        change(&mut todo);
        todo.id = id.clone();
        let completed = before.status != Status::Done && todo.status == Status::Done;
        if let (true, Some(recur)) = (completed, todo.recur.take()) {
            let today = todo.done.unwrap_or_else(|| Local::now().date_naive());
            let mut next = recur.follow_up(&todo, today);
            next.id = Id(Uuid::now_v7().to_string());
            let note_id: String =
                tx.query_row("SELECT note_id FROM todos WHERE id = ?1", [&id.0], |row| {
                    row.get(0)
                })?;
            insert(&tx, &note_id, &next, &now)?;
        }
        if todo != before {
            tx.execute(
                "UPDATE todos SET text = ?2, status = ?3, priority = ?4, due = ?5,
                 done = ?6, tags = ?7, pinned = ?8, recur = ?9, updated_at = ?10 WHERE id = ?1",
                params![
                    todo.id.0,
                    todo.text,
                    to_sql(todo.status),
                    to_sql(todo.priority),
                    todo.due,
                    todo.done,
                    tags_to_sql(&todo.tags),
                    todo.pinned,
                    recur_to_sql(todo.recur),
                    now,
                ],
            )?;
        }
        tx.commit()?;
        Ok(todo)
    }

    /// Marks a todo deleted. Returns it.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no todo has this id, [`Error::Db`] on failure.
    pub fn remove(&mut self, id: &Id) -> Result<Todo> {
        let tx = self.conn.transaction()?;
        let todo = get(&tx, id)?.ok_or_else(|| Error::NotFound(id.clone()))?;
        let now = now();
        tx.execute(
            "UPDATE todos SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id.0, now],
        )?;
        tx.commit()?;
        Ok(todo)
    }

    /// Copies every todo of an Obsidian vault into the database, in one
    /// transaction. Never writes to the vault.
    ///
    /// Safe to run again: todos whose id is already in the database are
    /// skipped. A task without a `^id` gets one made from its note, line
    /// position and text, so it is skipped the next time too.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the vault can't be read, [`Error::Db`] on failure.
    /// Nothing is imported then.
    pub fn import_vault(&mut self, root: &Path) -> Result<Imported> {
        let mut paths = Vec::new();
        find_notes(root, Path::new(""), &mut paths)?;
        paths.sort();
        let mut report = Imported::default();
        let now = now();
        let tx = self.conn.transaction()?;
        for path in paths {
            let full = root.join(&path);
            let src =
                fs::read_to_string(&full).map_err(|source| Error::Io { path: full, source })?;
            let note = path
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let note_id = ensure_note(&tx, &note, &now)?;
            report.notes += 1;
            for (n, mut todo) in todo_md::parse_note(&src).into_iter().enumerate() {
                if todo.id.0.is_empty() {
                    let seed = format!("{note}\n{n}\n{}", todo.text);
                    todo.id = Id(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()).to_string());
                }
                if id_taken(&tx, &todo.id)? {
                    report.skipped += 1;
                } else {
                    insert(&tx, &note_id, &todo, &now)?;
                    report.added += 1;
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }
}

/// What [`Store::import_vault`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Imported {
    /// Notes read.
    pub notes: usize,
    /// Todos added.
    pub added: usize,
    /// Todos skipped because their id was already in the database.
    pub skipped: usize,
}

/// Runs the migrations this database hasn't run yet.
fn migrate(conn: &mut Connection) -> Result<()> {
    let done: usize = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let tx = conn.transaction()?;
    for (n, sql) in MIGRATIONS.iter().enumerate().skip(done) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", n + 1)?;
    }
    tx.commit()?;
    Ok(())
}

/// The todo with this id, unless deleted.
fn get(conn: &Connection, id: &Id) -> Result<Option<Todo>> {
    let sql = format!(
        "SELECT {TODO_COLUMNS} FROM todos t JOIN notes n ON n.id = t.note_id
         WHERE t.id = ?1 AND t.deleted_at IS NULL"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([&id.0], read_todo)
        .optional()?)
}

/// `true` if any todo, deleted ones too, has this id.
fn id_taken(tx: &Transaction<'_>, id: &Id) -> Result<bool> {
    Ok(tx
        .prepare_cached("SELECT 1 FROM todos WHERE id = ?1")?
        .exists([&id.0])?)
}

/// The id of the note with this name, created (or undeleted) if needed.
fn ensure_note(tx: &Transaction<'_>, name: &str, now: &str) -> Result<String> {
    let found: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT id, deleted_at FROM notes WHERE name = ?1",
            [name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match found {
        Some((id, None)) => Ok(id),
        Some((id, Some(_))) => {
            tx.execute(
                "UPDATE notes SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
            Ok(id)
        }
        None => {
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO notes (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                params![id, name, now],
            )?;
            Ok(id)
        }
    }
}

/// Inserts a todo. Its created date, if any, becomes `created_at`.
fn insert(tx: &Transaction<'_>, note_id: &str, todo: &Todo, now: &str) -> Result<()> {
    let created_at = todo.created.map_or_else(|| now.to_owned(), local_midnight);
    tx.execute(
        "INSERT INTO todos (id, note_id, text, status, priority, due, done, tags, pinned, recur,
                            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            todo.id.0,
            note_id,
            todo.text,
            to_sql(todo.status),
            to_sql(todo.priority),
            todo.due,
            todo.done,
            tags_to_sql(&todo.tags),
            todo.pinned,
            recur_to_sql(todo.recur),
            created_at,
            now,
        ],
    )?;
    Ok(())
}

/// Reads [`TODO_COLUMNS`] into a todo.
fn read_todo(row: &Row<'_>) -> rusqlite::Result<Todo> {
    let created_at: String = row.get(7)?;
    let tags: String = row.get(6)?;
    let recur: Option<String> = row.get(10)?;
    Ok(Todo {
        id: Id(row.get(0)?),
        text: row.get(1)?,
        status: from_sql(row, 2)?,
        priority: from_sql(row, 3)?,
        due: row.get(4)?,
        done: row.get(5)?,
        tags: serde_json::from_str(&tags).map_err(|e| bad_column(6, e))?,
        created: Some(
            DateTime::parse_from_rfc3339(&created_at)
                .map_err(|e| bad_column(7, e))?
                .with_timezone(&Local)
                .date_naive(),
        ),
        pinned: row.get(9)?,
        recur: recur
            .map(|recur| serde_json::from_str(&recur))
            .transpose()
            .map_err(|e| bad_column(10, e))?,
    })
}

/// A recurrence as JSON, like `{"every":2,"unit":"week"}`.
fn recur_to_sql(recur: Option<Recur>) -> Option<String> {
    recur.and_then(|recur| serde_json::to_string(&recur).ok())
}

/// A unit enum as its serde name, like `in_progress`.
fn to_sql(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// Reverse of [`to_sql`].
fn from_sql<T: DeserializeOwned>(row: &Row<'_>, column: usize) -> rusqlite::Result<T> {
    let name: String = row.get(column)?;
    serde_json::from_value(serde_json::Value::String(name)).map_err(|e| bad_column(column, e))
}

/// Tags as a JSON list.
fn tags_to_sql(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_owned())
}

/// A column that holds something we can't read.
fn bad_column(
    column: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(error))
}

/// Now, as stored: RFC 3339 in UTC with milliseconds, so it sorts as text.
fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Midnight of `date` in the server's time zone, as stored.
fn local_midnight(date: NaiveDate) -> String {
    let local = Local
        .from_local_datetime(&date.and_time(chrono::NaiveTime::MIN))
        .earliest()
        .map_or_else(
            || date.and_time(chrono::NaiveTime::MIN).and_utc(),
            |t| t.with_timezone(&Utc),
        );
    local.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Refuses names that look like paths leaving a folder, hidden ones, and
/// names not ending in `.md`, so they stay what the old vault had.
fn check_note(note: &str) -> Result<()> {
    let path = Path::new(note);
    let clean = !note.contains('\\')
        && path.components().all(|part| match part {
            Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
            _ => false,
        });
    if clean && path.extension().is_some_and(|ext| ext == "md") {
        Ok(())
    } else {
        Err(Error::BadNote(note.to_owned()))
    }
}

/// Collects `.md` files under `root/dir` as paths relative to `root`.
/// Skips hidden entries (`.obsidian`, `.trash`) and symlinks.
fn find_notes(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let full = root.join(dir);
    let io_err = |source| Error::Io {
        path: full.clone(),
        source,
    };
    for entry in fs::read_dir(&full).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = dir.join(&name);
        let kind = entry.file_type().map_err(io_err)?;
        if kind.is_dir() {
            find_notes(root, &path, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use todo_core::{Priority, Unit};

    use super::*;

    /// Shorthand for ids.
    fn id(s: &str) -> Id {
        Id(s.to_owned())
    }

    /// "Today" in these tests.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 25).unwrap_or_default()
    }

    /// An empty store with `todo.md` as inbox.
    fn store() -> Store {
        Store::open_in_memory("todo.md").expect("store")
    }

    /// A temp vault holding `files` as (path, content).
    fn vault(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (path, content) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().expect("has a parent")).expect("mkdir");
            fs::write(path, content).expect("write");
        }
        dir
    }

    #[test]
    fn add_gives_a_uuid_and_round_trips_every_field() -> Result<()> {
        let mut store = store();
        let mut todo = Todo::new(Id(String::new()), "buy milk", today());
        todo.priority = Priority::High;
        todo.due = NaiveDate::from_ymd_opt(2026, 10, 30);
        todo.tags = vec!["errand".into(), "dev/rust".into()];
        let saved = store.add(todo.clone())?;
        assert!(Uuid::parse_str(&saved.id.0).is_ok(), "{}", saved.id.0);
        todo.id = saved.id.clone();
        assert_eq!(saved, todo);
        assert_eq!(store.get(&saved.id)?, Some(todo));
        assert_eq!(store.notes()?, ["todo.md"]);
        Ok(())
    }

    #[test]
    fn notes_are_created_on_first_todo_and_listed_in_order() -> Result<()> {
        let mut store = store();
        store.add_to("projects/dev.md", Todo::new(id("b"), "b", today()))?;
        store.add_to("a.md", Todo::new(id("a"), "a", today()))?;
        store.add_to("projects/dev.md", Todo::new(id("c"), "c", today()))?;
        assert_eq!(store.notes()?, ["a.md", "projects/dev.md"]);
        let listed: Vec<(String, String)> = store
            .todos_by_note(None)?
            .into_iter()
            .map(|(note, todo)| (note, todo.id.0))
            .collect();
        assert_eq!(
            listed,
            [
                ("a.md".into(), "a".into()),
                ("projects/dev.md".into(), "b".into()),
                ("projects/dev.md".into(), "c".into()),
            ]
        );
        assert_eq!(store.todos_by_note(Some("a.md"))?.len(), 1);
        Ok(())
    }

    #[test]
    fn bad_note_names_and_taken_ids_are_refused() -> Result<()> {
        let mut store = store();
        for note in [
            "../out.md",
            "/tmp/out.md",
            ".obsidian/x.md",
            "notes.txt",
            "",
            "a\\b.md",
        ] {
            assert!(
                matches!(
                    store.add_to(note, Todo::new(id("x"), "x", today())),
                    Err(Error::BadNote(_))
                ),
                "{note}"
            );
        }
        store.add(Todo::new(id("a"), "a", today()))?;
        store.remove(&id("a"))?;
        // Even a deleted todo keeps its id.
        assert!(matches!(
            store.add(Todo::new(id("a"), "again", today())),
            Err(Error::DuplicateId(_))
        ));
        Ok(())
    }

    #[test]
    fn update_changes_fields_but_not_the_id() -> Result<()> {
        let mut store = store();
        store.add(Todo::new(id("a"), "a", today()))?;
        let saved = store.update(&id("a"), |t| {
            t.complete(today());
            t.id = id("other");
        })?;
        assert_eq!(saved.id, id("a"));
        assert_eq!(saved.status, Status::Done);
        assert_eq!(store.get(&id("a"))?, Some(saved));
        assert!(matches!(
            store.update(&id("zzz"), |_| {}),
            Err(Error::NotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn pinned_and_recur_are_saved() -> Result<()> {
        let mut store = store();
        let mut todo = Todo::new(id("a"), "a", today());
        todo.pinned = true;
        todo.recur = Some(Recur {
            every: 2,
            unit: Unit::Month,
        });
        assert_eq!(store.add(todo.clone())?, todo);
        let saved = store.update(&id("a"), |t| {
            t.pinned = false;
            t.recur = None;
        })?;
        assert_eq!((saved.pinned, saved.recur), (false, None));
        assert_eq!(store.get(&id("a"))?, Some(saved));
        Ok(())
    }

    #[test]
    fn completing_a_recurring_todo_adds_the_next_one_once() -> Result<()> {
        let mut store = store();
        let mut todo = Todo::new(id("a"), "water plants", today());
        todo.due = NaiveDate::from_ymd_opt(2026, 10, 20);
        todo.recur = Some(Recur {
            every: 1,
            unit: Unit::Week,
        });
        store.add_to("home.md", todo)?;
        let done = store.update(&id("a"), |t| t.complete(today()))?;
        assert_eq!((done.status, done.recur), (Status::Done, None));
        // Completing again, or reopening and completing, adds nothing more.
        store.update(&id("a"), |t| t.complete(today()))?;
        store.update(&id("a"), Todo::reopen)?;
        store.update(&id("a"), |t| t.complete(today()))?;
        let todos = store.todos_by_note(Some("home.md"))?;
        assert_eq!(todos.len(), 2);
        let next = &todos[1].1;
        assert_eq!(next.text, "water plants");
        assert_eq!(next.status, Status::Todo);
        assert_eq!(next.due, NaiveDate::from_ymd_opt(2026, 10, 27));
        assert!(next.recur.is_some());
        Ok(())
    }

    #[test]
    fn remove_hides_the_todo_but_keeps_the_row() -> Result<()> {
        let mut store = store();
        store.add(Todo::new(id("a"), "a", today()))?;
        assert_eq!(store.remove(&id("a"))?.text, "a");
        assert_eq!(store.get(&id("a"))?, None);
        assert!(store.todos_by_note(None)?.is_empty());
        assert!(matches!(store.remove(&id("a")), Err(Error::NotFound(_))));
        let rows: i64 = store.conn.query_row(
            "SELECT count(*) FROM todos WHERE deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn reopening_the_file_keeps_everything() -> Result<()> {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("state/todo.db");
        let todo = Store::open(&path, "todo.md")?.add(Todo::new(id("a"), "a", today()))?;
        assert_eq!(Store::open(&path, "todo.md")?.get(&id("a"))?, Some(todo));
        Ok(())
    }

    #[test]
    fn import_copies_the_vault_once() -> Result<()> {
        let dir = vault(&[
            (
                "todo.md",
                "# Todo\n- [ ] a 📅 2026-10-30 #dev ^aaaaaa\n- [x] b ✅ 2026-10-20 ^bbbbbb\n",
            ),
            (
                "projects/dev.md",
                "- [/] c ⏫ ➕ 2026-10-01 ^cccccc\n- [ ] no id\n",
            ),
            ("empty.md", "Just text.\n"),
            (".obsidian/x.md", "- [ ] hidden ^hidden\n"),
        ]);
        let mut store = store();
        let first = store.import_vault(dir.path())?;
        assert_eq!(
            first,
            Imported {
                notes: 3,
                added: 4,
                skipped: 0
            }
        );
        assert_eq!(store.notes()?, ["empty.md", "projects/dev.md", "todo.md"]);
        let a = store.get(&id("aaaaaa"))?.expect("a");
        assert_eq!(a.due, NaiveDate::from_ymd_opt(2026, 10, 30));
        assert_eq!(a.tags, ["dev"]);
        let b = store.get(&id("bbbbbb"))?.expect("b");
        assert_eq!(b.done, NaiveDate::from_ymd_opt(2026, 10, 20));
        let c = store.get(&id("cccccc"))?.expect("c");
        assert_eq!(c.status, Status::InProgress);
        assert_eq!(c.created, NaiveDate::from_ymd_opt(2026, 10, 1));

        let again = store.import_vault(dir.path())?;
        assert_eq!(
            again,
            Imported {
                notes: 3,
                added: 0,
                skipped: 4
            }
        );
        assert_eq!(store.todos_by_note(None)?.len(), 4);
        Ok(())
    }
}
