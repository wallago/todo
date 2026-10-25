//! Main application.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use todo_store::Store;

use crate::prelude::*;

/// State shared by every route.
#[derive(Clone, Debug)]
pub(crate) struct AppState {
    /// The vault. The lock makes requests touch it one at a time.
    store: Arc<Mutex<Store>>,
}

impl AppState {
    /// Loads the vault.
    pub(crate) fn open(root: &Path, inbox: &Path) -> Result<Self> {
        Ok(Self::new(Store::open(root, inbox)?))
    }

    /// Wraps an already open store.
    pub(crate) fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }

    /// Locks the store.
    pub(crate) fn store(&self) -> MutexGuard<'_, Store> {
        // Store changes are all-or-nothing, so a panic can't leave it half-changed.
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
