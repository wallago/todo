//! Reads todos from Obsidian markdown notes (Tasks plugin format).
//!
//! Read-only, `&str` in: only `todo-server import` uses it, to move an old
//! vault into the database.
//!
//! ```
//! let todos = todo_md::parse_note("# Todo\n\n- [ ] buy milk 📅 2026-10-30 #errand ^k3x9a2\n");
//! assert_eq!(todos[0].text, "buy milk");
//! assert_eq!(todos[0].id.0, "k3x9a2");
//! ```

/// A whole note.
mod document;
/// One task line.
mod line;

pub use document::parse_note;
pub use line::parse_line;
