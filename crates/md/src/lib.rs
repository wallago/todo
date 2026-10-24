//! Read and write todos in Obsidian markdown notes (Tasks plugin format).
//!
//! Pure `&str` in, `String` out: the server owns the files and calls this.
//!
//! ```
//! use todo_md::Document;
//!
//! let mut doc = Document::parse("# Todo\n\n- [ ] buy milk 📅 2026-10-30 #errand ^k3x9a2\n");
//! let id = todo_core::Id("k3x9a2".into());
//! # let today = chrono::NaiveDate::default();
//! doc.update(&id, |todo| todo.complete(today))?;
//! assert!(doc.to_string().contains("- [x] buy milk"));
//! # Ok::<(), todo_md::Error>(())
//! ```

/// A whole note.
mod document;
/// One task line.
mod line;

pub use document::Document;
pub use line::{parse_line, write_line};
use todo_core::Id;

/// What can go wrong when changing a [`Document`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No todo has this id.
    #[error("no todo with id `{}`", .0.0)]
    NotFound(Id),
    /// Another todo already has this id.
    #[error("id `{}` is already used", .0.0)]
    DuplicateId(Id),
    /// The todo has an empty id.
    #[error("todo has no id")]
    MissingId,
}
