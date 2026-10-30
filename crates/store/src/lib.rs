//! Owns the vault: loads every note, finds todos by id, writes changes back.
//!
//! Every write re-reads its note first, so an edit made in Obsidian since the
//! last load is kept, not overwritten.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use todo_core::{Id, Todo};
use todo_md::Document;

/// What can go wrong in the store.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading or writing a file failed.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        source: io::Error,
    },
    /// The change was refused: unknown id, id already taken...
    #[error(transparent)]
    Md(#[from] todo_md::Error),
    /// The path is not a `.md` note inside the vault.
    #[error("`{}` is not a note in the vault: give a relative path to a .md file, like `projects/dev.md`", .0.display())]
    BadNote(PathBuf),
}

/// Shorthand for the store's results.
pub type Result<T> = std::result::Result<T, Error>;

/// Every note of a vault, kept in memory.
#[derive(Debug)]
pub struct Store {
    /// Vault folder.
    root: PathBuf,
    /// Note that new todos go to, relative to `root`.
    inbox: PathBuf,
    /// Every note, by path relative to `root`.
    notes: BTreeMap<PathBuf, Document>,
    /// Which note holds each todo.
    index: HashMap<Id, PathBuf>,
}

impl Store {
    /// Loads every `.md` note under `root`, skipping hidden folders like
    /// `.obsidian`. New todos go to `inbox`, a path relative to `root`.
    ///
    /// Tasks without an id get one, written to their note, so every todo can
    /// be addressed.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the folder or a note can't be read or written.
    pub fn open(root: impl Into<PathBuf>, inbox: impl Into<PathBuf>) -> Result<Self> {
        let mut store = Self {
            root: root.into(),
            inbox: inbox.into(),
            notes: BTreeMap::new(),
            index: HashMap::new(),
        };
        let mut paths = Vec::new();
        find_notes(&store.root, Path::new(""), &mut paths)?;
        paths.sort();
        for path in paths {
            store.reload(&path)?;
        }
        Ok(store)
    }

    /// Loads the whole vault again, e.g. after notes were added, renamed or
    /// deleted outside the server. Keeps the old notes if it fails.
    ///
    /// # Errors
    ///
    /// As [`Store::open`].
    pub fn rescan(&mut self) -> Result<()> {
        *self = Self::open(self.root.clone(), self.inbox.clone())?;
        Ok(())
    }

    /// Every todo, note by note.
    pub fn todos(&self) -> impl Iterator<Item = &Todo> {
        self.notes.values().flat_map(Document::todos)
    }

    /// Every todo with its note's path, relative to the vault.
    pub fn todos_by_note(&self) -> impl Iterator<Item = (&Path, &Todo)> {
        self.notes
            .iter()
            .flat_map(|(path, doc)| doc.todos().map(move |todo| (path.as_path(), todo)))
    }

    /// Every note's path, relative to the vault, todos or not.
    pub fn notes(&self) -> impl Iterator<Item = &Path> {
        self.notes.keys().map(PathBuf::as_path)
    }

    /// The todo with this id.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Todo> {
        self.notes.get(self.index.get(id)?)?.get(id)
    }

    /// Adds a todo to the inbox note, giving it an id if it has none.
    /// Returns it as saved.
    ///
    /// # Errors
    ///
    /// [`Error::Md`] if its id is already used, [`Error::Io`] if the inbox
    /// can't be read or written.
    pub fn add(&mut self, todo: Todo) -> Result<Todo> {
        let inbox = self.inbox.clone();
        self.insert(&inbox, todo)
    }

    /// Adds a todo to `note`, a path relative to the vault, creating the
    /// note if it is missing. Otherwise like [`Store::add`].
    ///
    /// # Errors
    ///
    /// [`Error::BadNote`] if `note` is not a `.md` file inside the vault or
    /// is hidden, then as [`Store::add`].
    pub fn add_to(&mut self, note: impl AsRef<Path>, todo: Todo) -> Result<Todo> {
        let note = note.as_ref();
        check_note(note)?;
        self.insert(note, todo)
    }

    /// Changes a todo and saves its note. Returns it as saved.
    ///
    /// # Errors
    ///
    /// [`Error::Md`] if no todo has this id or the change gives it a taken
    /// id, [`Error::Io`] if its note can't be read or written.
    pub fn update(&mut self, id: &Id, change: impl FnOnce(&mut Todo)) -> Result<Todo> {
        let path = self.locate(id)?;
        self.apply(&path, |doc| doc.update(id, change).cloned())
    }

    /// Removes a todo and its line. Returns it.
    ///
    /// # Errors
    ///
    /// [`Error::Md`] if no todo has this id, [`Error::Io`] if its note can't
    /// be read or written.
    pub fn remove(&mut self, id: &Id) -> Result<Todo> {
        let path = self.locate(id)?;
        self.apply(&path, |doc| doc.remove(id))
    }

    /// Reads one note again, e.g. after it was edited in Obsidian. `path` is
    /// relative to the vault. A note that no longer exists is dropped.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the note can't be read, or its new ids can't be
    /// written.
    pub fn reload(&mut self, path: &Path) -> Result<()> {
        let full = self.root.join(path);
        let src = match fs::read_to_string(&full) {
            Ok(src) => src,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.set_note(path, None);
                return Ok(());
            }
            Err(source) => return Err(Error::Io { path: full, source }),
        };
        let mut doc = Document::parse(&src);
        if self.fix_ids(path, &mut doc) > 0 {
            write_note(&full, &doc)?;
        }
        self.set_note(path, Some(doc));
        Ok(())
    }

    /// Adds a todo to `path`, which the caller trusts.
    fn insert(&mut self, path: &Path, mut todo: Todo) -> Result<Todo> {
        self.reload(path)?;
        if todo.id.0.is_empty() {
            todo.id = new_id(&mut self.index.keys().cloned().collect());
        }
        self.apply(path, |doc| doc.add(todo.clone()).map(|()| todo))
    }

    /// Re-reads the note holding `id` and returns its path.
    fn locate(&mut self, id: &Id) -> Result<PathBuf> {
        let not_found = || Error::Md(todo_md::Error::NotFound(id.clone()));
        let path = self.index.get(id).cloned().ok_or_else(not_found)?;
        self.reload(&path)?;
        // It may have been deleted in Obsidian meanwhile.
        match self.index.get(id) {
            Some(found) if *found == path => Ok(path),
            _ => Err(not_found()),
        }
    }

    /// Runs `change` on a copy of a note, saves it, then keeps it.
    /// Nothing changes if `change` or the write fails.
    fn apply<T>(
        &mut self,
        path: &Path,
        change: impl FnOnce(&mut Document) -> std::result::Result<T, todo_md::Error>,
    ) -> Result<T> {
        // A missing note starts empty.
        let mut doc = self
            .notes
            .get(path)
            .cloned()
            .unwrap_or_else(|| Document::parse(""));
        let out = change(&mut doc)?;
        if let Some(todo) = doc.todos().find(|t| self.in_other_note(&t.id, path)) {
            return Err(todo_md::Error::DuplicateId(todo.id.clone()).into());
        }
        write_note(&self.root.join(path), &doc)?;
        self.set_note(path, Some(doc));
        Ok(out)
    }

    /// Gives a fresh id to tasks with none, or with one another note already
    /// uses. Returns how many changed.
    fn fix_ids(&self, path: &Path, doc: &mut Document) -> usize {
        let mut taken: HashSet<Id> = self.index.keys().cloned().collect();
        taken.extend(doc.todos().map(|t| t.id.clone()));
        let mut changed = doc.ensure_ids(|| new_id(&mut taken));
        let copies: Vec<Id> = doc
            .todos()
            .filter(|t| self.in_other_note(&t.id, path))
            .map(|t| t.id.clone())
            .collect();
        for id in copies {
            let fresh = new_id(&mut taken);
            if doc.update(&id, |t| t.id = fresh).is_ok() {
                changed += 1;
            }
        }
        changed
    }

    /// `true` if a note other than `path` holds this id.
    fn in_other_note(&self, id: &Id, path: &Path) -> bool {
        self.index.get(id).is_some_and(|found| found != path)
    }

    /// Keeps a note in memory, or drops it, and updates the index.
    fn set_note(&mut self, path: &Path, doc: Option<Document>) {
        self.index.retain(|_, found| found != path);
        match doc {
            Some(doc) => {
                for todo in doc.todos() {
                    self.index.insert(todo.id.clone(), path.to_owned());
                }
                self.notes.insert(path.to_owned(), doc);
            }
            None => {
                self.notes.remove(path);
            }
        }
    }
}

/// Collects `.md` files under `root/dir` as paths relative to `root`.
/// Skips hidden entries (`.obsidian`, `.trash`, our temp files) and symlinks.
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

/// Refuses paths that leave the vault, hidden ones (`.obsidian`), and
/// files that are not `.md`, the same notes [`find_notes`] would skip.
fn check_note(note: &Path) -> Result<()> {
    let inside = note.components().all(|part| match part {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    if inside && note.extension().is_some_and(|ext| ext == "md") {
        Ok(())
    } else {
        Err(Error::BadNote(note.to_owned()))
    }
}

/// Writes a note to a temp file then renames it over the old one, so a crash
/// never leaves half a note.
fn write_note(path: &Path, doc: &Document) -> Result<()> {
    let io_err = |source| Error::Io {
        path: path.to_owned(),
        source,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let mut file = fs::File::create(&tmp).map_err(io_err)?;
    file.write_all(doc.to_string().as_bytes()).map_err(io_err)?;
    file.sync_all().map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)
}

/// Characters used in ids.
const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// A random 6-character id not in `taken`; it is added to `taken`.
fn new_id(taken: &mut HashSet<Id>) -> Id {
    loop {
        let mut n = RandomState::new().hash_one(taken.len());
        let id: String = (0..6)
            .map(|_| {
                let c = ALPHABET[usize::try_from(n % 36).unwrap_or_default()];
                n /= 36;
                char::from(c)
            })
            .collect();
        let id = Id(id);
        if taken.insert(id.clone()) {
            return id;
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    /// A temp vault holding `files` as (path, content).
    fn vault(files: &[(&str, &str)]) -> TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for (path, content) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().expect("has a parent")).expect("mkdir");
            fs::write(path, content).expect("write");
        }
        dir
    }

    /// A note's content on disk.
    fn read(dir: &TempDir, path: &str) -> String {
        fs::read_to_string(dir.path().join(path)).expect("read")
    }

    /// Shorthand for ids.
    fn id(s: &str) -> Id {
        Id(s.to_owned())
    }

    /// "Today" in these tests.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 25).unwrap_or_default()
    }

    #[test]
    fn loads_notes_and_skips_hidden_and_other_files() -> Result<()> {
        let dir = vault(&[
            ("todo.md", "- [ ] a ^aaaaaa\n"),
            ("projects/dev.md", "- [ ] b ^bbbbbb\n"),
            (".obsidian/plugin.md", "- [ ] hidden ^cccccc\n"),
            ("notes.txt", "- [ ] not markdown ^dddddd\n"),
        ]);
        let store = Store::open(dir.path(), "todo.md")?;
        let ids: Vec<&str> = store.todos().map(|t| t.id.0.as_str()).collect();
        assert_eq!(ids, ["bbbbbb", "aaaaaa"]);
        let notes: Vec<&Path> = store.notes().collect();
        assert_eq!(notes, [Path::new("projects/dev.md"), Path::new("todo.md")]);
        let (note, todo) = store.todos_by_note().next().expect("a todo");
        assert_eq!(
            (note, todo.text.as_str()),
            (Path::new("projects/dev.md"), "b")
        );
        assert_eq!(store.get(&id("bbbbbb")).map(|t| t.text.as_str()), Some("b"));
        Ok(())
    }

    #[test]
    fn missing_ids_are_written_to_the_note() -> Result<()> {
        let dir = vault(&[("todo.md", "# Todo\n\n- [ ] no id\n- [ ] has id ^aaaaaa\n")]);
        let store = Store::open(dir.path(), "todo.md")?;
        let new = store.todos().next().expect("a todo");
        assert_eq!(new.id.0.len(), 6);
        assert_eq!(
            read(&dir, "todo.md"),
            format!(
                "# Todo\n\n- [ ] no id ^{}\n- [ ] has id ^aaaaaa\n",
                new.id.0
            )
        );
        Ok(())
    }

    #[test]
    fn copy_in_another_note_gets_a_new_id() -> Result<()> {
        let dir = vault(&[
            ("a.md", "- [ ] first ^aaaaaa\n"),
            ("b.md", "- [ ] copy ^aaaaaa\n"),
        ]);
        let store = Store::open(dir.path(), "a.md")?;
        // a.md loads first, so it keeps the id.
        assert_eq!(
            store.get(&id("aaaaaa")).map(|t| t.text.as_str()),
            Some("first")
        );
        assert_eq!(read(&dir, "a.md"), "- [ ] first ^aaaaaa\n");
        // The copy in b.md gets a new one, saved to the note.
        let copy = store.todos().find(|t| t.text == "copy").expect("copy");
        assert_ne!(copy.id, id("aaaaaa"));
        assert_eq!(read(&dir, "b.md"), format!("- [ ] copy ^{}\n", copy.id.0));
        Ok(())
    }

    #[test]
    fn add_goes_to_the_inbox_and_creates_it() -> Result<()> {
        let dir = vault(&[("other.md", "- [ ] x ^aaaaaa\n")]);
        let mut store = Store::open(dir.path(), "inbox/todo.md")?;
        let todo = store.add(Todo::new(Id(String::new()), "buy milk", today()))?;
        assert_eq!(todo.id.0.len(), 6);
        assert_eq!(
            read(&dir, "inbox/todo.md"),
            format!("- [ ] buy milk ➕ 2026-10-25 ^{}\n", todo.id.0)
        );
        assert_eq!(store.get(&todo.id), Some(&todo));
        Ok(())
    }

    #[test]
    fn add_to_another_note_creates_it() -> Result<()> {
        let dir = vault(&[("todo.md", "- [ ] x ^aaaaaa\n")]);
        let mut store = Store::open(dir.path(), "todo.md")?;
        let todo = store.add_to(
            "projects/dev.md",
            Todo::new(Id(String::new()), "ship it", today()),
        )?;
        assert_eq!(
            read(&dir, "projects/dev.md"),
            format!("- [ ] ship it ➕ 2026-10-25 ^{}\n", todo.id.0)
        );
        assert_eq!(read(&dir, "todo.md"), "- [ ] x ^aaaaaa\n");
        assert_eq!(store.get(&todo.id), Some(&todo));
        Ok(())
    }

    #[test]
    fn add_to_refuses_notes_outside_the_vault() -> Result<()> {
        let dir = vault(&[("todo.md", "")]);
        let mut store = Store::open(dir.path(), "todo.md")?;
        for note in [
            "../out.md",
            "/tmp/out.md",
            ".obsidian/x.md",
            "notes.txt",
            "",
        ] {
            assert!(
                matches!(
                    store.add_to(note, Todo::new(Id(String::new()), "x", today())),
                    Err(Error::BadNote(_))
                ),
                "{note}"
            );
        }
        assert_eq!(store.todos().count(), 0);
        Ok(())
    }

    #[test]
    fn update_keeps_edits_made_in_obsidian() -> Result<()> {
        let dir = vault(&[("todo.md", "- [ ] a ^aaaaaa\n")]);
        let mut store = Store::open(dir.path(), "todo.md")?;
        // Obsidian adds a line after we loaded.
        fs::write(
            dir.path().join("todo.md"),
            "- [ ] a ^aaaaaa\n- [ ] b ^bbbbbb\n",
        )
        .expect("write");
        store.update(&id("aaaaaa"), |t| t.complete(today()))?;
        assert_eq!(
            read(&dir, "todo.md"),
            "- [x] a ✅ 2026-10-25 ^aaaaaa\n- [ ] b ^bbbbbb\n"
        );
        assert!(store.get(&id("bbbbbb")).is_some());
        Ok(())
    }

    #[test]
    fn remove_deletes_the_line() -> Result<()> {
        let dir = vault(&[("todo.md", "# Todo\n- [ ] a ^aaaaaa\n- [ ] b ^bbbbbb\n")]);
        let mut store = Store::open(dir.path(), "todo.md")?;
        assert_eq!(store.remove(&id("aaaaaa"))?.text, "a");
        assert_eq!(read(&dir, "todo.md"), "# Todo\n- [ ] b ^bbbbbb\n");
        assert_eq!(store.get(&id("aaaaaa")), None);
        Ok(())
    }

    #[test]
    fn rescan_picks_up_new_moved_and_deleted_notes() -> Result<()> {
        let dir = vault(&[("a.md", "- [ ] a ^aaaaaa\n"), ("b.md", "- [ ] b ^bbbbbb\n")]);
        let mut store = Store::open(dir.path(), "a.md")?;
        fs::create_dir(dir.path().join("trips")).expect("mkdir");
        fs::rename(dir.path().join("a.md"), dir.path().join("trips/a.md")).expect("mv");
        fs::remove_file(dir.path().join("b.md")).expect("rm");
        fs::write(dir.path().join("c.md"), "- [ ] c ^cccccc\n").expect("write");
        store.rescan()?;
        let notes: Vec<&Path> = store.notes().collect();
        assert_eq!(notes, [Path::new("c.md"), Path::new("trips/a.md")]);
        // A moved todo keeps its id.
        assert_eq!(store.get(&id("aaaaaa")).map(|t| t.text.as_str()), Some("a"));
        assert_eq!(store.get(&id("bbbbbb")), None);
        Ok(())
    }

    #[test]
    fn unknown_or_taken_ids_are_refused() -> Result<()> {
        let dir = vault(&[("a.md", "- [ ] a ^aaaaaa\n"), ("b.md", "- [ ] b ^bbbbbb\n")]);
        let mut store = Store::open(dir.path(), "a.md")?;
        assert!(matches!(
            store.update(&id("zzzzzz"), |_| {}),
            Err(Error::Md(todo_md::Error::NotFound(_)))
        ));
        assert!(matches!(
            store.update(&id("bbbbbb"), |t| t.id = id("aaaaaa")),
            Err(Error::Md(todo_md::Error::DuplicateId(_)))
        ));
        assert_eq!(read(&dir, "b.md"), "- [ ] b ^bbbbbb\n");
        assert!(store.get(&id("bbbbbb")).is_some());
        Ok(())
    }
}
