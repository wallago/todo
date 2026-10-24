use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span, Text},
    widgets::{Block, BorderType::Rounded, Paragraph},
};
use tui_input::Input;

use super::prelude::*;
use crate::ui::state::mode::Mode;

/// One page renderer per mode.
mod mode;
/// Left column: mode navigation and summary.
mod sidebar;

/// Renders the user interface widgets.
pub(super) fn render(state: &mut State, frame: &mut Frame) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("praline ", ASCENT).bold(),
        Span::styled("- repo builder", SECONDARY),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(Rounded)
        .border_style(ASCENT)
        .fg(PRIMARY)
        .bg(BACKGROUND)
        .title(title)
        .title_alignment(Alignment::Left);
    frame.render_widget(block, frame.area());
    {
        let chunks = Layout::new(
            Direction::Horizontal,
            [Constraint::Length(32), Constraint::Min(0)],
        )
        .margin(2)
        .spacing(2)
        .split(frame.area());
        sidebar::render(state, frame, chunks[0]);
        match state.mode {
            Mode::Dashboard | Mode::Settings => mode::dashboard::render(state, frame, chunks[1]),
            Mode::Details => mode::details::render(state, frame, chunks[1]),
        }
    }
    if let Some(status) = &state.status {
        render_status(status, frame, frame.area());
    } else {
        render_key_bindings(state, frame, frame.area());
    }
}

/// Renders the key bindings.
fn render_key_bindings(state: &mut State, frame: &mut Frame, rect: Rect) {
    let chunks = Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(rect);
    let key_bindings = state.get_key_bindings();
    let line = Line::from(
        key_bindings
            .iter()
            .enumerate()
            .flat_map(|(i, (keys, desc))| {
                vec![
                    "[".fg(UNASCENT),
                    keys.clone().fg(PRIMARY),
                    "→ ".fg(UNASCENT),
                    Span::from(*desc).fg(SECONDARY),
                    "]".fg(UNASCENT),
                    if i == key_bindings.len() - 1 { "" } else { " " }.into(),
                ]
            })
            .collect::<Vec<Span>>(),
    );
    let Ok(width) = u16::try_from(line.width()) else {
        return;
    };
    if width > chunks[1].width.saturating_sub(25) {
        return;
    }
    frame.render_widget(Paragraph::new(line.alignment(Alignment::Center)), chunks[1]);
}

/// Renders the last action's outcome on the key bindings' row.
fn render_status(status: &Status, frame: &mut Frame, rect: Rect) {
    let chunks = Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(rect);
    let line = match status {
        Status::Info(message) => Line::from(format!(" {} ", message.as_str())).fg(SECONDARY),
        Status::Error(message) => Line::from(format!(" {} ", message.as_str()))
            .fg(ERROR)
            .bold(),
    };
    frame.render_widget(Paragraph::new(line.alignment(Alignment::Center)), chunks[1]);
}

/// Draws button.
fn render_button(
    frame: &mut Frame,
    rect: Rect,
    title: &str,
    value: &str,
    focused: bool,
    error: Option<&str>,
) {
    let bordered = Block::bordered().border_type(Rounded);
    let block = match error {
        Some(error) => {
            let title = format!(" {title} · {error} ").fg(ERROR).bold();
            bordered.title(title).border_style(ERROR)
        }
        None if focused => {
            let title = format!(" {title} ").fg(ASCENT).bold();
            bordered.title(title).border_style(ASCENT)
        }
        None => {
            let title = format!(" {title} ").fg(SECONDARY).bold();
            bordered.title(title).border_style(ASCENT_BIS)
        }
    };
    let area = block.inner(rect);
    frame.render_widget(block, rect);
    let (paragraph, _) = {
        let (rows, at) = wrap(value, 0, area.width, area.height);
        (Paragraph::new(Text::from_iter(rows)), at)
    };
    frame.render_widget(paragraph, area);
}

/// A single bordered text field. `editing` is the live buffer while the field
/// is typed into, and `error` says why that buffer can't be saved yet. A field
/// one row tall scrolls sideways; a taller one wraps.
fn render_input(
    frame: &mut Frame,
    rect: Rect,
    title: &str,
    value: &str,
    focused: bool,
    editing: Option<&Input>,
    error: Option<&str>,
) {
    let bordered = Block::bordered().border_type(Rounded);
    let block = match (editing, error) {
        (Some(_), Some(error)) => {
            let title = format!(" {title} · {error} ").fg(ERROR).bold();
            bordered.title(title).border_style(ERROR)
        }
        (Some(_), None) => {
            let title = format!(" {title} ").fg(HIGHLIGHT).bold();
            bordered.title(title).border_style(HIGHLIGHT)
        }
        (None, _) if focused => {
            let title = format!(" {title} ").fg(ASCENT).bold();
            bordered.title(title).border_style(ASCENT)
        }
        (None, _) => {
            let title = format!(" {title} ").fg(SECONDARY).bold();
            bordered.title(title).border_style(ASCENT_BIS)
        }
    };
    let area = block.inner(rect);
    frame.render_widget(block, rect);
    let text = editing.map_or(value, Input::value);
    let (paragraph, (col, row)) = if area.height > 1 {
        let cursor = editing.map_or(0, Input::cursor);
        let (rows, at) = wrap(text, cursor, area.width, area.height);
        (Paragraph::new(Text::from_iter(rows)), at)
    } else {
        // Last column stays free so the cursor has somewhere to sit at the end.
        let width = usize::from(area.width.saturating_sub(1));
        let scroll = editing.map_or(0, |input| input.visual_scroll(width));
        let col = editing
            .map_or(0, Input::visual_cursor)
            .saturating_sub(scroll);
        let shift = u16::try_from(scroll).unwrap_or_default();
        let paragraph = Paragraph::new(text).scroll((0, shift));
        (paragraph, (u16::try_from(col).unwrap_or_default(), 0))
    };
    frame.render_widget(paragraph, area);
    if editing.is_some() {
        frame.set_cursor_position((area.x + col, area.y + row));
    }
}

/// Hard-wraps `text` at `width` columns and scrolls so the `cursor`-th char
/// stays within `height` rows. Returns the rows to draw and the cursor's
/// `(col, row)` inside the box. One char per column, so a wide glyph (CJK,
/// emoji) pushes the cursor off.
fn wrap(text: &str, cursor: usize, width: u16, height: u16) -> (Vec<String>, (u16, u16)) {
    let width = usize::from(width.max(1));
    let (col, row) = (cursor % width, cursor / width);
    let top = row.saturating_sub(usize::from(height.max(1)) - 1);
    let chars: Vec<char> = text.chars().collect();
    let rows = chars
        .chunks(width)
        .skip(top)
        .map(|row| row.iter().collect())
        .collect();
    // Both fit: `col` is under `width` and `row - top` under `height`.
    let at = (
        u16::try_from(col).unwrap_or_default(),
        u16::try_from(row - top).unwrap_or_default(),
    );
    (rows, at)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::wrap;

    #[rstest]
    #[case::cursor_mid_text("abcdefg", 4, 3, (vec!["abc", "def", "g"], (1, 1)))]
    #[case::cursor_past_full_row("abcdef", 6, 3, (vec!["abc", "def"], (0, 2)))]
    #[case::scrolls_to_cursor("abcdefgh", 8, 2, (vec!["def", "gh"], (2, 1)))]
    fn wraps_and_keeps_the_cursor_in_view(
        #[case] text: &str,
        #[case] cursor: usize,
        #[case] height: u16,
        #[case] expected: (Vec<&str>, (u16, u16)),
    ) {
        let (rows, at) = wrap(text, cursor, 3, height);
        assert_eq!((rows.iter().map(String::as_str).collect(), at), expected);
    }
}
