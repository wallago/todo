//! The one-line popup: add or change a todo, or confirm a delete.

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use super::pane;
use crate::tui::{
    state::{Ask, Prompt},
    theme::{ACCENT, BACKGROUND, ERROR, PRIMARY},
};

/// The text being typed, or the todo about to be deleted.
pub(super) fn render(prompt: &Prompt, frame: &mut Frame, page: Rect) {
    let area = page.centered(Constraint::Percentage(80), Constraint::Length(4));
    frame.render_widget(Clear, area);
    let line = if matches!(prompt.ask, Ask::Delete(_)) {
        Line::from(Span::styled(prompt.input.as_str(), ERROR))
    } else {
        Line::from(vec![
            Span::styled("> ", ACCENT),
            Span::styled(prompt.input.as_str(), PRIMARY),
            Span::styled("▏", ACCENT),
        ])
    };
    let block = pane(prompt.title(), true).bg(BACKGROUND);
    frame.render_widget(Paragraph::new(line).block(block), area);
}
