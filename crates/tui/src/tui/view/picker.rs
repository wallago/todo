//! The note finder popup, centred over the page.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::Clear,
};

use super::{list, note_name, pane};
use crate::tui::{
    state::Picker,
    theme::{ACCENT, BACKGROUND, PRIMARY},
};

/// A query line, then the matching notes.
pub(super) fn render(picker: &mut Picker, frame: &mut Frame, page: Rect) {
    let area = page.centered(Constraint::Percentage(70), Constraint::Percentage(70));
    frame.render_widget(Clear, area);
    let block = pane("find note", true).bg(BACKGROUND);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [query, _, matches] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);
    let line = Line::from(vec![
        Span::styled("> ", ACCENT),
        Span::styled(picker.query.as_str(), PRIMARY),
        Span::styled("▏", ACCENT),
    ]);
    frame.render_widget(line, query);
    if let Some(note) = picker.new_note() {
        let hint = Line::from(vec![
            Span::styled("+ new note ", ACCENT),
            Span::styled(note_name(&note).to_owned(), PRIMARY),
        ]);
        frame.render_widget(hint, matches);
        return;
    }
    let names = list(
        picker.matches.iter().map(|note| note_name(note)),
        ratatui::widgets::Block::new(),
    );
    frame.render_stateful_widget(names, matches, &mut picker.list);
}
