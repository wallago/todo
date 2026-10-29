//! Widget renderer.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Gauge, HighlightSpacing, List, ListState, Padding, Paragraph},
};

use super::{
    state::{Focus, Page, State},
    theme::{ACCENT, ACCENT_BIS, BACKGROUND, HIGHLIGHT, PRIMARY, SECONDARY, UNACCENT},
};

/// Renders the whole screen: frame, sidebar, page and key bindings.
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
    match state.page {
        Page::Home => render_home(state, frame, page),
        Page::About => render_about(state, frame, page),
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
fn list<'a>(items: impl IntoIterator<Item = &'a str>, block: Block<'a>) -> List<'a> {
    List::new(items)
        .block(block)
        .style(SECONDARY)
        .highlight_style(Style::new().fg(ACCENT).bg(ACCENT_BIS).bold())
        .highlight_symbol(Span::styled("▌ ", ACCENT))
        .highlight_spacing(HighlightSpacing::Always)
}

/// Page navigation on top, a summary of the home list below.
fn render_sidebar(state: &State, frame: &mut Frame, rect: Rect) {
    let [navigate, summary] = Layout::vertical([Constraint::Ratio(1, 2); 2]).areas(rect);
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
    let at = state.list.selected().map_or(0, |at| at.saturating_add(1));
    let len = state.items.len();
    frame.render_widget(Line::from(Span::styled("ITEMS", SECONDARY)), header);
    frame.render_widget(
        Line::from(Span::styled(format!("{at}/{len} selected"), PRIMARY)).bold(),
        content,
    );
    #[expect(clippy::cast_precision_loss, reason = "a list fits in an f64")]
    let ratio = if len == 0 {
        0.0
    } else {
        at as f64 / len as f64
    };
    let gauge = Gauge::default()
        .gauge_style(Style::new().fg(ACCENT).bg(ACCENT_BIS))
        .ratio(ratio.clamp(0.0, 1.0))
        .label("")
        .use_unicode(true);
    frame.render_widget(gauge, bar);
}

/// The home page: a list to move through.
fn render_home(state: &mut State, frame: &mut Frame, rect: Rect) {
    let items = list(
        state.items.iter().copied(),
        pane("home", state.focus == Focus::Page),
    );
    frame.render_stateful_widget(items, rect, &mut state.list);
}

/// The about page: what this crate is.
fn render_about(state: &State, frame: &mut Frame, rect: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled(env!("CARGO_PKG_NAME"), ACCENT).bold(),
            Span::raw(" "),
            Span::styled(env!("CARGO_PKG_VERSION"), HIGHLIGHT),
        ]),
        Line::default(),
        Line::from(Span::styled(env!("CARGO_PKG_DESCRIPTION"), PRIMARY)),
    ];
    let about = Paragraph::new(text).block(pane("about", state.focus == Focus::Page));
    frame.render_widget(about, rect);
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

    #[test]
    fn home_shows_the_title_the_list_and_the_keys() {
        let screen = screen(&mut State::default());
        assert!(screen.contains(env!("CARGO_PKG_NAME")));
        assert!(screen.contains("first item"));
        assert!(screen.contains("1/3 selected"));
        assert!(screen.contains("quit"));
    }

    #[test]
    fn about_shows_the_version() {
        let mut state = State {
            page: Page::About,
            ..State::default()
        };
        assert!(screen(&mut state).contains(env!("CARGO_PKG_VERSION")));
    }
}
