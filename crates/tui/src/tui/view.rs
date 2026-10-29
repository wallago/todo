//! Widget renderer.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Gauge, HighlightSpacing, List, Padding, Paragraph, Wrap},
};
use todo_client::Todo;

use super::{
    state::{Focus, State},
    theme::{ACCENT, ACCENT_BIS, BACKGROUND, ERROR, HIGHLIGHT, PRIMARY, SECONDARY, UNACCENT},
};

/// Renders the whole screen: frame, notes, todos and key bindings.
pub(super) fn render(state: &mut State, frame: &mut Frame) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(concat!(env!("CARGO_PKG_NAME"), " "), ACCENT).bold(),
        Span::styled(concat!("- ", env!("CARGO_PKG_DESCRIPTION")), SECONDARY),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(ACCENT)
        .fg(PRIMARY)
        .bg(BACKGROUND)
        .title(title);
    frame.render_widget(block, frame.area());
    let [sidebar, page] = Layout::horizontal([Constraint::Length(32), Constraint::Min(0)])
        .margin(2)
        .spacing(2)
        .areas(frame.area());
    render_sidebar(state, frame, sidebar);
    render_todos(state, frame, page);
    render_key_bindings(state, frame, frame.area());
}

/// A rounded pane titled `title`, lit up while `focused`.
fn pane(title: &str, focused: bool) -> Block<'_> {
    let (title_color, border) = if focused {
        (ACCENT, ACCENT)
    } else {
        (SECONDARY, ACCENT_BIS)
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(format!(" {title} ")).fg(title_color).bold())
        .border_style(border)
        .padding(Padding::new(1, 1, 1, 0))
}

/// A list styled like praline's: muted rows, the selected one lit with a bar.
fn list<'a>(items: impl IntoIterator<Item = &'a str>, block: Block<'a>) -> List<'a> {
    List::new(items)
        .block(block)
        .style(SECONDARY)
        .highlight_style(Style::new().fg(ACCENT).bg(ACCENT_BIS).bold())
        .highlight_symbol(Span::styled("▌ ", ACCENT))
        .highlight_spacing(HighlightSpacing::Always)
}

/// Notes on top, how much of the selected one is done below.
fn render_sidebar(state: &mut State, frame: &mut Frame, rect: Rect) {
    let [notes, summary] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(7)]).areas(rect);
    let names = list(
        state.notes.iter().map(|note| note_name(note)),
        pane("notes", state.focus == Focus::Notes),
    );
    frame.render_stateful_widget(names, notes, &mut state.note_list);

    let block = pane("summary", false);
    let inner = block.inner(summary);
    frame.render_widget(block, summary);
    let [header, content, _, bar] = Layout::vertical([Constraint::Length(1); 4]).areas(inner);
    let done = state
        .todos
        .iter()
        .filter(|todo| todo.status.is_closed())
        .count();
    let len = state.todos.len();
    frame.render_widget(Line::from(Span::styled("TODOS", SECONDARY)), header);
    frame.render_widget(
        Line::from(Span::styled(format!("{done}/{len} done"), PRIMARY)).bold(),
        content,
    );
    #[expect(clippy::cast_precision_loss, reason = "a list fits in an f64")]
    let ratio = if len == 0 {
        0.0
    } else {
        done as f64 / len as f64
    };
    let gauge = Gauge::default()
        .gauge_style(Style::new().fg(ACCENT).bg(ACCENT_BIS))
        .ratio(ratio.clamp(0.0, 1.0))
        .label("")
        .use_unicode(true);
    frame.render_widget(gauge, bar);
}

/// A note path without its `.md`, like `projects/todo`.
fn note_name(note: &str) -> &str {
    note.strip_suffix(".md").unwrap_or(note)
}

/// The selected note's todos, or the last server error.
fn render_todos(state: &mut State, frame: &mut Frame, rect: Rect) {
    let title = state.note().map_or("todos", note_name).to_owned();
    let block = pane(&title, state.focus == Focus::Todos);
    if let Some(error) = &state.error {
        let text = Paragraph::new(Line::from(Span::styled(error.as_str(), ERROR)))
            .wrap(Wrap { trim: true })
            .block(block);
        frame.render_widget(text, rect);
        return;
    }
    let rows = state.todos.iter().map(todo_row);
    let todos = List::new(rows)
        .block(block)
        .style(SECONDARY)
        .highlight_style(Style::new().bg(ACCENT_BIS).bold())
        .highlight_symbol(Span::styled("▌ ", ACCENT))
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(todos, rect, &mut state.todo_list);
}

/// `[ ] text ⏫ 2026-11-02`, dimmed once closed.
fn todo_row(todo: &Todo) -> Line<'_> {
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

/// The keys that work right now, centred on the bottom border.
fn render_key_bindings(state: &State, frame: &mut Frame, rect: Rect) {
    let [_, row] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(rect);
    let bindings = state.key_bindings();
    let mut spans = Vec::new();
    for (i, (keys, desc)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.extend([
            "[".fg(UNACCENT),
            keys.fg(PRIMARY),
            "→ ".fg(UNACCENT),
            desc.fg(SECONDARY),
            "]".fg(UNACCENT),
        ]);
    }
    let line = Line::from(spans).alignment(Alignment::Center);
    frame.render_widget(Paragraph::new(line), row);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, buffer::Cell};
    use todo_client::{Id, Status};

    use super::*;

    /// Draws `state` on an 80×24 screen and returns its text, row after row.
    fn screen(state: &mut State) -> String {
        let mut terminal = match Terminal::new(TestBackend::new(80, 24)) {
            Ok(terminal) => terminal,
            Err(never) => match never {},
        };
        let _ = terminal.draw(|frame| render(state, frame));
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
            .collect()
    }

    /// A todo with `text` and `status`, the rest left default.
    fn todo(id: &str, text: &str, status: Status) -> Todo {
        let mut todo = Todo::new(Id(id.into()), text, chrono::NaiveDate::MIN);
        todo.status = status;
        todo
    }

    #[test]
    fn shows_notes_todos_and_keys() {
        let mut state = State::default();
        state.set_notes(vec!["projects/todo.md".into(), "travel.md".into()]);
        state.set_todos(vec![
            todo("a", "write the tui", Status::Todo),
            todo("b", "auth", Status::Done),
        ]);
        let screen = screen(&mut state);
        assert!(screen.contains(env!("CARGO_PKG_NAME")));
        assert!(screen.contains("projects/todo"));
        assert!(!screen.contains("todo.md"));
        assert!(screen.contains("[ ] write the tui"));
        assert!(screen.contains("[x] auth"));
        assert!(screen.contains("1/2 done"));
        assert!(screen.contains("quit"));
    }

    #[test]
    fn shows_the_server_error() {
        let mut state = State {
            error: Some("server is down".into()),
            ..State::default()
        };
        assert!(screen(&mut state).contains("server is down"));
    }
}
