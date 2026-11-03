//! The note finder popup, centred over the page.

use chrono::NaiveDate;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Clear, ListItem},
};

use super::{list, note_name, pane};
use crate::tui::{
    state::{NoteStats, Picker},
    theme::{ACCENT, BACKGROUND, HIGHLIGHT, PRIMARY},
};

/// A query line, then the matching notes, each with when its next open
/// todo is due on the right.
pub(super) fn render(picker: &mut Picker, stats: &[NoteStats], frame: &mut Frame, page: Rect) {
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
    let today = chrono::Local::now().date_naive();
    // Less the `▌ ` highlight bar.
    let width = usize::from(matches.width).saturating_sub(2);
    let rows = picker.matches.iter().map(|note| {
        let name = note_name(note);
        let due = stats
            .iter()
            .find(|row| &row.note == note)
            .and_then(|row| row.next_due)
            .map(|due| due_label(due, today));
        row(name, due, width)
    });
    let names = list(rows, ratatui::widgets::Block::new());
    frame.render_stateful_widget(names, matches, &mut picker.list);
}

/// `name`, and `due` pushed to the right edge of `width` in highlight.
fn row(name: &str, due: Option<String>, width: usize) -> ListItem<'_> {
    let Some(due) = due else {
        return ListItem::new(name);
    };
    let used = name.chars().count().saturating_add(due.chars().count());
    let gap = width.saturating_sub(used).max(1);
    ListItem::new(Line::from(vec![
        Span::raw(name),
        Span::raw(" ".repeat(gap)),
        Span::styled(due, HIGHLIGHT),
    ]))
}

/// When `due` is, seen from `today`: `2d late`, `today`, `tomorrow`, `in 5d`.
fn due_label(due: NaiveDate, today: NaiveDate) -> String {
    match (due - today).num_days() {
        days if days < 0 => format!("{}d late", days.unsigned_abs()),
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        days => format!("in {days}d"),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::late(-2, "2d late")]
    #[case::today(0, "today")]
    #[case::tomorrow(1, "tomorrow")]
    #[case::later(5, "in 5d")]
    fn labels_the_due_date(#[case] days: i64, #[case] label: &str) {
        let today = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap_or_default();
        let due = today + chrono::Duration::days(days);
        assert_eq!(due_label(due, today), label);
    }
}
