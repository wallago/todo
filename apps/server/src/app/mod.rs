//! Main application.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use todo_store::Store;

use crate::prelude::*;

/// State shared by every route.
#[derive(Clone, Debug)]
pub(crate) struct AppState {
    /// The database. The lock makes requests touch it one at a time.
    store: Arc<Mutex<Store>>,
}

impl AppState {
    /// Opens the database.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        Ok(Self::new(Store::open(path)?))
    }

    /// Wraps an already open store.
    pub(crate) fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }

    /// Locks the store.
    pub(crate) fn store(&self) -> MutexGuard<'_, Store> {
        // Store changes are transactions, so a panic can't leave one half done.
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
