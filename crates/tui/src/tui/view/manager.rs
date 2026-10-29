//! The manager page: one note's todos.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};
use todo_client::Todo;

use super::{list, note_name, pane};
use crate::tui::{
    state::Manager,
    theme::{ACCENT, ACCENT_BIS, HIGHLIGHT, PRIMARY, SECONDARY},
};

/// The open note's todos.
pub(super) fn render(manager: &mut Manager, focused: bool, frame: &mut Frame, rect: Rect) {
    let title = manager
        .note
        .as_deref()
        .map_or("no note", note_name)
        .to_owned();
    let todos = list(manager.todos.iter().map(row), pane(&title, focused))
        // Rows colour themselves; the selection only adds a background.
        .highlight_style(Style::new().bg(ACCENT_BIS).bold());
    frame.render_stateful_widget(todos, rect, &mut manager.list);
}

/// `[ ] text ⏫ 2026-11-02`, dimmed once closed.
fn row(todo: &Todo) -> Line<'_> {
    let text = if todo.status.is_closed() {
        SECONDARY
    } else {
        PRIMARY
    };
    let mut spans = vec![
        Span::styled(format!("[{}] ", todo.status.as_char()), ACCENT),
        Span::styled(todo.text.as_str(), text),
    ];
    if let Some(emoji) = todo.priority.emoji() {
        spans.push(Span::raw(format!(" {emoji}")));
    }
    if let Some(due) = todo.due {
        spans.push(Span::styled(format!(" {due}"), HIGHLIGHT));
    }
    Line::from(spans)
}
