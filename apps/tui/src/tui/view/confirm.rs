//! The `y/n` popup before deleting a todo.

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use super::pane;
use crate::tui::{
    state::Confirm,
    theme::{BACKGROUND, ERROR},
};

/// The todo about to be deleted.
pub(super) fn render(confirm: &Confirm, frame: &mut Frame, page: Rect) {
    let area = page.centered(Constraint::Percentage(80), Constraint::Length(4));
    frame.render_widget(Clear, area);
    let line = Line::from(Span::styled(confirm.text.as_str(), ERROR));
    let block = pane("delete todo? y/n", true).bg(BACKGROUND);
    frame.render_widget(Paragraph::new(line).block(block), area);
}
