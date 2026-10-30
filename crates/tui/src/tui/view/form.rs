//! The add and edit form: fields on the left, the focused field's
//! dropdown or calendar on the right.

use chrono::{Datelike, Days, NaiveDate};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, ListState, Padding, Paragraph},
};
use todo_client::{Priority, Status};

use super::{list, pane};
use crate::tui::{
    state::{Field, Form, PRIORITIES, STATUSES},
    theme::{ACCENT, ACCENT_BIS, BACKGROUND, HIGHLIGHT, PRIMARY, SECONDARY},
};

/// The whole form, centred over the page.
pub(super) fn render(form: &Form, frame: &mut Frame, page: Rect) {
    let area = page.centered(Constraint::Max(70), Constraint::Length(15));
    frame.render_widget(Clear, area);
    let title = if form.is_edit() {
        "edit todo"
    } else {
        "new todo"
    };
    let block = pane(title, true).bg(BACKGROUND);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [fields, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(26)])
        .spacing(2)
        .areas(inner);
    render_fields(form, frame, fields);
    match form.field {
        Field::Title => {}
        Field::Priority => {
            let at = PRIORITIES.iter().position(|&p| p == form.priority);
            let names = PRIORITIES.iter().map(|&p| priority_name(p));
            dropdown(names, at, frame, side);
        }
        Field::Due => calendar(form.due.unwrap_or(form.today), form, frame, side),
        Field::Status => {
            let at = STATUSES.iter().position(|&s| s == form.status);
            let names = STATUSES.iter().map(|&s| status_name(s));
            dropdown(names, at, frame, side);
        }
    }
}

/// One rounded box per field, titled with its name, lit while focused.
fn render_fields(form: &Form, frame: &mut Frame, rect: Rect) {
    let rows = Layout::vertical([Constraint::Length(3); 4]).split(rect);
    let due = form
        .due
        .map_or_else(|| "none".to_owned(), |due| due.to_string());
    let values = [
        form.title.clone(),
        priority_name(form.priority),
        due,
        status_name(form.status),
    ];
    for ((field, value), row) in Field::ALL.iter().zip(values).zip(rows.iter()) {
        let focused = *field == form.field;
        let (title, border) = if focused {
            (ACCENT, ACCENT)
        } else {
            (SECONDARY, ACCENT_BIS)
        };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(border)
            .title(Line::from(format!(" {} ", label(*field))).fg(title).bold())
            .padding(Padding::horizontal(1));
        let mut spans = vec![Span::styled(value, PRIMARY)];
        if focused && *field == Field::Title {
            spans.push(Span::styled("▏", ACCENT));
        } else if *field != Field::Title {
            spans.push(Span::styled(" ▾", SECONDARY));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).block(block), *row);
    }
}

/// The field's name, on its box.
fn label(field: Field) -> &'static str {
    match field {
        Field::Title => "title",
        Field::Priority => "priority",
        Field::Due => "due",
        Field::Status => "status",
    }
}

/// `⏫ high`, or plain `normal`.
fn priority_name(priority: Priority) -> String {
    let name = match priority {
        Priority::Lowest => "lowest",
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Highest => "highest",
    };
    priority
        .emoji()
        .map_or_else(|| name.to_owned(), |emoji| format!("{emoji} {name}"))
}

/// `[/] in progress`.
fn status_name(status: Status) -> String {
    let name = match status {
        Status::Todo => "todo",
        Status::InProgress => "in progress",
        Status::Done => "done",
        Status::Cancelled => "cancelled",
    };
    format!("[{}] {name}", status.as_char())
}

/// The open dropdown: every option, the current one lit.
fn dropdown(names: impl Iterator<Item = String>, at: Option<usize>, frame: &mut Frame, rect: Rect) {
    let mut selected = ListState::default().with_selected(at);
    frame.render_stateful_widget(list(names, pane("choose", true)), rect, &mut selected);
}

/// The month of `day`, weeks from Monday; `day` lit, today in amber.
fn calendar(day: NaiveDate, form: &Form, frame: &mut Frame, rect: Rect) {
    let month = day.format("%B %Y").to_string();
    let block = pane(&month, true);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let mut lines = vec![Line::from(Span::styled("Mo Tu We Th Fr Sa Su", SECONDARY))];
    let first = day.with_day(1).unwrap_or(day);
    let offset = first.weekday().num_days_from_monday();
    let mut spans = vec![Span::raw(
        "   ".repeat(usize::try_from(offset).unwrap_or(0)),
    )];
    let mut date = first;
    while date.month() == day.month() {
        let style = if Some(date) == form.due {
            Style::new().fg(BACKGROUND).bg(ACCENT).bold()
        } else if date == form.today {
            Style::new().fg(HIGHLIGHT)
        } else {
            Style::new().fg(PRIMARY)
        };
        spans.push(Span::styled(format!("{:>2}", date.day()), style));
        if date.weekday().num_days_from_monday() == 6 {
            lines.push(Line::from(std::mem::take(&mut spans)));
        } else {
            spans.push(Span::raw(" "));
        }
        let Some(next) = date.checked_add_days(Days::new(1)) else {
            break;
        };
        date = next;
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    if form.due.is_none() {
        lines.push(Line::from(Span::styled("no due date", ACCENT_BIS)));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
