//! The stats page: how much of each note is done.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::LineGauge,
};

use super::{note_name, pane, ratio};
use crate::tui::{
    state::NoteStats,
    theme::{ACCENT, ACCENT_BIS, PRIMARY},
};

/// One gauge per note, as many as fit.
pub(super) fn render(stats: &[NoteStats], focused: bool, frame: &mut Frame, rect: Rect) {
    let block = pane("stats", focused);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let width = stats
        .iter()
        .map(|row| note_name(&row.note).len())
        .max()
        .unwrap_or(0);
    let rows = Layout::vertical(vec![Constraint::Length(1); stats.len()]).split(inner);
    for (row, area) in stats.iter().zip(rows.iter()) {
        let label = format!(
            "{:<width$}  {:>3}/{:<3} ",
            note_name(&row.note),
            row.done,
            row.total
        );
        let gauge = LineGauge::default()
            .label(label)
            .style(Style::new().fg(PRIMARY))
            .filled_style(Style::new().fg(ACCENT))
            .unfilled_style(Style::new().fg(ACCENT_BIS))
            .ratio(ratio(row.done, row.total));
        frame.render_widget(gauge, *area);
    }
}
