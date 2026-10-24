use std::{cell::LazyCell, ffi::OsStr, path::Path};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, HighlightSpacing, List, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};
use syntect_assets::assets::HighlightingAssets;
use tui_syntax_highlight::Highlighter;

use crate::ui::{prelude::*, state::mode::DetailsPane};

thread_local! {
    /// Syntax definitions and themes, loaded on first use.
    static ASSETS: LazyCell<HighlightingAssets> = LazyCell::new(HighlightingAssets::from_binary);
}

/// Draws the details page.
pub(in crate::ui::view) fn render(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Ratio(1, 3), Constraint::Min(0)],
    )
    .spacing(2)
    .split(chunk);
    {
        row_1(state, frame, chunks[0]);
        row_2(state, frame, chunks[1]);
    }
}

/// Draws row 1.
fn row_1(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("staged files", SECONDARY).bold(),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .padding(Padding::new(1, 1, 1, 0))
        .border_style(border(state, DetailsPane::List));
    let files: Vec<Line> = state
        .app
        .staged
        .as_ref()
        .map(|tree| {
            tree.keys()
                .map(|path| Line::from(path.to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    if state.details.files.selected().is_none() && !files.is_empty() {
        state.details.files.select_first();
    }
    let list = List::new(files)
        .block(block)
        .style(SECONDARY)
        .highlight_style(Style::new().fg(ASCENT).bg(ASCENT_BIS).bold())
        .highlight_symbol(Span::styled("▌ ", ASCENT))
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(list, chunk, &mut state.details.files);
}

/// Draws row 2.
fn row_2(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let chunks = Layout::new(Direction::Vertical, [Constraint::Min(0)]).split(chunk);

    {
        let at = state.details.files.selected().unwrap_or_default();
        let selected = state
            .app
            .staged
            .as_ref()
            .and_then(|tree| tree.iter().nth(at));

        let (name, body) = match selected {
            Some((path, bytes)) => (
                path.display().to_string(),
                highlight(&path.to_string_lossy(), &String::from_utf8_lossy(bytes)),
            ),
            None => (String::from("-"), Text::raw("nothing staged")),
        };
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(name, SECONDARY).bold(),
            Span::raw(" "),
        ]);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .padding(Padding::new(1, 1, 1, 0))
            .border_style(border(state, DetailsPane::Content));

        let overflow = body.height().saturating_sub(usize::from(chunks[0].height));
        state.details.scroll = state
            .details
            .scroll
            .min(u16::try_from(overflow).unwrap_or(u16::MAX));
        frame.render_widget(
            Paragraph::new(body)
                .scroll((state.details.scroll, 0))
                .block(block),
            chunks[0],
        );
        if overflow > 0 {
            let mut bar = ScrollbarState::new(overflow + 1)
                .viewport_content_length(usize::from(chunks[0].height))
                .position(usize::from(state.details.scroll));
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .thumb_style(border(state, DetailsPane::Content))
                    .track_style(border(state, DetailsPane::Content)),
                chunks[0].inner(Margin::new(0, 1)),
                &mut bar,
            );
        }
    }
}

/// Border colour for `pane`: accented when it owns the keyboard.
fn border(state: &State, pane: DetailsPane) -> Color {
    if state.focus == Focus::Page && state.details.focus == pane {
        ASCENT
    } else {
        ASCENT_BIS
    }
}

/// Colors `content` by the syntax its path suggests, with line numbers.
fn highlight(path: &str, content: &str) -> Text<'static> {
    ASSETS.with(|assets| {
        let Ok(syntaxes) = assets.get_syntax_set() else {
            return Text::raw(content.to_owned());
        };
        let path = Path::new(path);
        let by_suffix = |name: Option<&OsStr>| {
            name.and_then(OsStr::to_str)
                .and_then(|name| syntaxes.find_syntax_by_extension(name))
        };
        let syntax = by_suffix(path.extension())
            .or_else(|| by_suffix(path.file_name()))
            .or_else(|| {
                content
                    .lines()
                    .next()
                    .and_then(|line| syntaxes.find_syntax_by_first_line(line))
            })
            .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
        Highlighter::new(assets.get_theme("ansi").clone())
            .line_number_style(Style::new().fg(SECONDARY))
            .line_number_separator_style(Style::new().fg(ASCENT_BIS))
            .highlight_reader(content.as_bytes(), syntax, syntaxes)
            .unwrap_or_else(|_| Text::raw(content.to_owned()))
    })
}
