//! The about page: what this crate is.

use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::pane;
use crate::tui::theme::{ACCENT, HIGHLIGHT, PRIMARY};

/// Name, version and description.
pub(super) fn render(focused: bool, frame: &mut Frame, rect: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled(env!("CARGO_PKG_NAME"), ACCENT).bold(),
            Span::raw(" "),
            Span::styled(env!("CARGO_PKG_VERSION"), HIGHLIGHT),
        ]),
        Line::default(),
        Line::from(Span::styled(env!("CARGO_PKG_DESCRIPTION"), PRIMARY)),
    ];
    frame.render_widget(Paragraph::new(text).block(pane("about", focused)), rect);
}
