//! Widget renderer: the frame, sidebar and key bindings here, one file per page.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Gauge, HighlightSpacing, List, ListState, Padding, Paragraph, Wrap,
    },
};

use super::{
    state::{Focus, Page, State},
    theme::{ACCENT, ACCENT_BIS, BACKGROUND, ERROR, PRIMARY, SECONDARY, UNACCENT},
};

/// The about page.
mod about;
/// The `y/n` popup before a delete.
mod confirm;
/// The add and edit form.
mod form;
/// The manager page.
mod manager;
/// The note finder popup.
mod picker;
/// The stats page.
mod stats;

/// Renders the whole screen: frame, sidebar, page, popup and key bindings.
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
    let [sidebar, page] = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)])
        .margin(2)
        .spacing(2)
        .areas(frame.area());
    render_sidebar(state, frame, sidebar);
    let focused = state.focus == Focus::Page;
    if let Some(error) = &state.error {
        render_error(error, frame, page);
    } else {
        match state.page {
            Page::Manager => manager::render(&mut state.manager, focused, frame, page),
            Page::Stats => stats::render(&state.stats, focused, frame, page),
            Page::About => about::render(focused, frame, page),
        }
    }
    if let Some(picker) = &mut state.picker {
        picker::render(picker, frame, page);
    }
    if let Some(form) = &state.form {
        form::render(form, frame, page);
    }
    if let Some(confirm) = &state.confirm {
        confirm::render(confirm, frame, page);
    }
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
fn list<'a, T: Into<ratatui::widgets::ListItem<'a>>>(
    items: impl IntoIterator<Item = T>,
    block: Block<'a>,
) -> List<'a> {
    List::new(items)
        .block(block)
        .style(SECONDARY)
        .highlight_style(Style::new().fg(ACCENT).bg(ACCENT_BIS).bold())
        .highlight_symbol(Span::styled("▌ ", ACCENT))
        .highlight_spacing(HighlightSpacing::Always)
}

/// A note path without its `.md`, like `projects/todo`.
fn note_name(note: &str) -> &str {
    note.strip_suffix(".md").unwrap_or(note)
}

/// Page navigation on top, how much of the open note is done below.
fn render_sidebar(state: &State, frame: &mut Frame, rect: Rect) {
    let [navigate, summary] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(7)]).areas(rect);
    let pages = list(
        Page::ALL.iter().map(|page| page.name()),
        pane("navigate", state.focus == Focus::Sidebar),
    );
    let mut selected = ListState::default().with_selected(Some(state.page.index()));
    frame.render_stateful_widget(pages, navigate, &mut selected);

    let block = pane("summary", false);
    let inner = block.inner(summary);
    frame.render_widget(block, summary);
    let [header, content, _, bar] = Layout::vertical([Constraint::Length(1); 4]).areas(inner);
    let (done, len) = state.manager.counts();
    frame.render_widget(Line::from(Span::styled("TODOS", SECONDARY)), header);
    frame.render_widget(
        Line::from(Span::styled(format!("{done}/{len} done"), PRIMARY)).bold(),
        content,
    );
    let gauge = Gauge::default()
        .gauge_style(Style::new().fg(ACCENT).bg(ACCENT_BIS))
        .ratio(ratio(done, len))
        .label("")
        .use_unicode(true);
    frame.render_widget(gauge, bar);
}

/// `done / total`, 0 for an empty note.
fn ratio(done: usize, total: usize) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a note's todos fit in an f64")]
    let ratio = if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    };
    ratio.clamp(0.0, 1.0)
}

/// The last server error, in place of the page.
fn render_error(error: &str, frame: &mut Frame, rect: Rect) {
    let text = Paragraph::new(Line::from(Span::styled(error, ERROR)))
        .wrap(Wrap { trim: true })
        .block(pane("error", false));
    frame.render_widget(text, rect);
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
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Cell,
        crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    };
    use todo_client::{Id, Status, Todo};

    use super::*;
    use crate::tui::state::NoteStats;

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

    /// A state with two notes, the first open with two todos.
    fn with_todos() -> State {
        let mut state = State::default();
        state.set_notes(vec!["projects/todo.md".into(), "travel.md".into()]);
        state.manager.set_todos(vec![
            todo("a", "write the tui", Status::Todo),
            todo("b", "auth", Status::Done),
        ]);
        state
    }

    #[test]
    fn manager_shows_the_note_its_todos_and_keys() {
        let screen = screen(&mut with_todos());
        assert!(screen.contains(env!("CARGO_PKG_NAME")));
        assert!(screen.contains("manager"));
        assert!(screen.contains("projects/todo"));
        assert!(!screen.contains("todo.md"));
        assert!(screen.contains("[ ] write the tui"));
        assert!(screen.contains("[x] auth"));
        assert!(screen.contains("1/2 done"));
        assert!(screen.contains("quit"));
    }

    #[test]
    fn picker_pops_up_over_the_manager() {
        let mut state = with_todos();
        state.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        state.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        let screen = screen(&mut state);
        assert!(screen.contains("find note"));
        assert!(screen.contains("> t"));
        assert!(screen.contains("travel"));
    }

    #[test]
    fn form_pops_up_with_the_typed_title() {
        let mut state = with_todos();
        state.focus = Focus::Page;
        state.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        state.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        let screen = screen(&mut state);
        assert!(screen.contains("new todo"));
        assert!(screen.contains("╭ title ─"));
        assert!(screen.contains("│ x▏"));
        assert!(screen.contains("normal ▾"));
        assert!(screen.contains("save"));
    }

    #[test]
    fn form_lists_tags_with_checkboxes() {
        let mut state = with_todos();
        state.tags = vec!["car".into(), "pet".into()];
        state.focus = Focus::Page;
        let press = |state: &mut State, code| state.on_key(KeyEvent::new(code, KeyModifiers::NONE));
        press(&mut state, KeyCode::Char('a'));
        press(&mut state, KeyCode::BackTab);
        press(&mut state, KeyCode::Char(' '));
        let shown = screen(&mut state);
        assert!(shown.contains("╭ tags ─"));
        assert!(shown.contains("[x] #car"));
        assert!(shown.contains("[ ] #pet"));
        assert!(shown.contains("│ #car"));
        press(&mut state, KeyCode::Char('z'));
        assert!(screen(&mut state).contains("+ new #z"));
    }

    #[test]
    fn picker_offers_a_new_note_when_nothing_matches() {
        let mut state = with_todos();
        state.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "zz/new".chars() {
            state.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert!(screen(&mut state).contains("+ new note zz/new"));
    }

    #[test]
    fn stats_shows_each_note() {
        let mut state = State {
            page: Page::Stats,
            stats: vec![NoteStats {
                note: "travel.md".into(),
                done: 1,
                total: 4,
            }],
            ..State::default()
        };
        let screen = screen(&mut state);
        assert!(screen.contains("travel"));
        assert!(screen.contains("1/4"));
    }

    #[test]
    fn about_shows_the_version() {
        let mut state = State {
            page: Page::About,
            ..State::default()
        };
        assert!(screen(&mut state).contains(env!("CARGO_PKG_VERSION")));
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
