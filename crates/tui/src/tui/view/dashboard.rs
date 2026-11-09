//! The dashboard page: three tiles on top, every due todo below.

use chrono::NaiveDate;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Paragraph},
};
use todo_client::Todo;

use super::{due_style, list, note_name, pane};
use crate::tui::{
    state::{Dashboard, DueTodo},
    theme::{ACCENT, ACCENT_BIS, HIGHLIGHT, PRIMARY, SECONDARY},
};

/// Today, done and open tiles, then the due list.
pub(super) fn render(
    dashboard: &mut Dashboard,
    today: NaiveDate,
    focused: bool,
    frame: &mut Frame,
    rect: Rect,
) {
    let [tiles, due] = Layout::vertical([Constraint::Length(4), Constraint::Min(0)])
        .spacing(1)
        .areas(rect);
    let [day, done, open] = Layout::horizontal([Constraint::Fill(1); 3])
        .spacing(2)
        .areas(tiles);
    let date = today.format("%Y/%m/%d").to_string();
    tile("TODAY", Span::styled(date, ACCENT), frame, day);
    let done_count = format!("✓ {} done", dashboard.done);
    tile("DONE", Span::styled(done_count, ACCENT), frame, done);
    let open_count = format!("{} to do", dashboard.open);
    tile(
        "STILL OPEN",
        Span::styled(open_count, HIGHLIGHT),
        frame,
        open,
    );

    let title = format!("due ({})", dashboard.due.len());
    if dashboard.due.is_empty() {
        let empty =
            Paragraph::new(Span::styled("Nothing due", SECONDARY)).block(pane(&title, focused));
        frame.render_widget(empty, due);
        return;
    }
    let width = dashboard
        .due
        .iter()
        .map(|due| label(&due.todo).width())
        .max()
        .unwrap_or(0);
    let rows = list(
        dashboard.due.iter().map(|due| row(due, today, width)),
        pane(&title, focused),
    )
    // Rows colour themselves; the selection only adds a background.
    .highlight_style(Style::new().bg(ACCENT_BIS).bold());
    frame.render_stateful_widget(rows, due, &mut dashboard.list);
}

/// A small box: a muted `label`, then `value` in bold.
fn tile(label: &str, value: Span<'_>, frame: &mut Frame, rect: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(ACCENT_BIS)
        .padding(Padding::horizontal(1));
    let text = Paragraph::new(vec![
        Line::from(Span::styled(label, SECONDARY)),
        Line::from(value.bold()),
    ])
    .block(block);
    frame.render_widget(text, rect);
}

/// `2026-11-11  text ⏫   note`, the date coloured by [`due_style`].
fn row(due: &DueTodo, today: NaiveDate, width: usize) -> Line<'_> {
    let todo = &due.todo;
    let date = due_style(todo, today);
    let mut spans = vec![
        Span::styled(
            todo.due.map(|day| day.to_string()).unwrap_or_default(),
            date,
        ),
        Span::raw("  "),
    ];
    let label = label(todo);
    let pad = width.saturating_sub(label.width());
    spans.extend(label.spans);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(
        format!("  {}", note_name(&due.note)),
        SECONDARY,
    ));
    Line::from(spans)
}

/// The todo's text, then its priority emoji if any.
fn label(todo: &Todo) -> Line<'_> {
    let mut spans = vec![Span::styled(todo.text.as_str(), PRIMARY)];
    if let Some(emoji) = todo.priority.emoji() {
        spans.push(Span::raw(format!(" {emoji}")));
    }
    Line::from(spans)
}
