//! Reloads the vault when Obsidian or Syncthing change it.

use std::path::{Component, Path};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use super::AppState;
use crate::prelude::*;

/// How long the vault must stay quiet before it is reloaded. Sync tools
/// write in bursts, so this waits for the end of one.
const QUIET: Duration = Duration::from_millis(500);

/// Watches the vault while it is alive.
pub(crate) type Watcher = Debouncer<RecommendedWatcher>;

impl AppState {
    /// Watches `root` and every folder below it. When a note is added,
    /// edited, renamed or deleted, the whole vault is loaded again.
    ///
    /// Our own writes are seen too: the reload then finds nothing new.
    pub(crate) fn watch(&self, root: &Path) -> Result<Watcher> {
        let state = self.clone();
        let root_owned = root.to_owned();
        let mut watcher = new_debouncer(QUIET, move |events: DebounceEventResult| {
            let events = match events {
                Ok(events) => events,
                Err(e) => {
                    tracing::warn!("watching the vault: {e}");
                    return;
                }
            };
            if !events.iter().any(|e| is_visible(&root_owned, &e.path)) {
                return;
            }
            match state.store().rescan() {
                Ok(()) => tracing::info!("vault changed on disk, reloaded"),
                Err(e) => tracing::warn!("reloading the vault: {e}"),
            }
        })?;
        watcher.watcher().watch(root, RecursiveMode::Recursive)?;
        Ok(watcher)
    }
}

/// `false` for paths the store skips anyway: `.obsidian`, `.trash`, our temp
/// files, Syncthing's `.syncthing.*.tmp`.
fn is_visible(root: &Path, path: &Path) -> bool {
    // Event paths may be absolute even when `root` is relative.
    let rel = path
        .strip_prefix(root)
        .or_else(|_| path.strip_prefix(root.canonicalize().unwrap_or_default()))
        .unwrap_or(path);
    rel.components().all(|part| match part {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Instant;

    use todo_store::Store;

    use super::*;

    #[test]
    fn hidden_paths_are_ignored() {
        let root = Path::new("/vault");
        assert!(is_visible(root, Path::new("/vault/trips/rome.md")));
        assert!(is_visible(root, Path::new("/vault/trips")));
        assert!(!is_visible(
            root,
            Path::new("/vault/.obsidian/workspace.json")
        ));
        assert!(!is_visible(root, Path::new("/vault/trips/.rome.md.tmp")));
    }

    #[test]
    fn new_notes_show_up_without_a_restart() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("todo.md"), "- [ ] a ^aaaaaa\n")?;
        let state = AppState::new(Store::open(dir.path(), "todo.md")?);
        let _watcher = state.watch(dir.path())?;

        fs::create_dir(dir.path().join("trips"))?;
        fs::write(dir.path().join("trips/rome.md"), "- [ ] pack ^bbbbbb\n")?;
        fs::remove_file(dir.path().join("todo.md"))?;

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let notes: Vec<_> = state.store().notes().map(Path::to_owned).collect();
            if notes == [Path::new("trips/rome.md")] {
                return Ok(());
            }
            assert!(Instant::now() < deadline, "vault not reloaded: {notes:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
