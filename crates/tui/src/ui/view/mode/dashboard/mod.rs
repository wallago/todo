use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

use crate::ui::{prelude::*, state::mode::DashboardPane};

/// Draws `form`.
mod form;
/// Draws `options`.
mod options;
/// Draws `picker`.
mod picker;

/// Draws the dashboard page.
pub(in crate::ui::view) fn render(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(65), Constraint::Percentage(35)],
    )
    .spacing(2)
    .split(chunk);
    {
        row_1(state, frame, chunks[0]);
        row_2(state, frame, chunks[1]);
    }
    picker::render_picker(state, frame);
}

/// Draws row 1.
fn row_1(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let chunks = Layout::new(
        Direction::Vertical,
        [Constraint::Percentage(65), Constraint::Percentage(35)],
    )
    .split(chunk);

    {
        options::render_options(state, frame, chunks[0]);
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled("tdf", SECONDARY).bold(),
            Span::raw(" "),
        ]);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title);
        // .border_style(border(state, DashboardPane::Tdf));
        frame.render_widget(block, chunks[1]);
    }
}

/// Draws row 2.
fn row_2(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let chunks = Layout::new(
        Direction::Vertical,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(60),
            Constraint::Percentage(25),
        ],
    )
    .split(chunk);

    {
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled("tdf", SECONDARY).bold(),
            Span::raw(" "),
        ]);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title);
        // .border_style(border(state, DashboardPane::Preview));
        frame.render_widget(block, chunks[0]);
        form::render_form(state, frame, chunks[1]);
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled("tdf", SECONDARY).bold(),
            Span::raw(" "),
        ]);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title);
        // .border_style(border(state, DashboardPane::Status));
        frame.render_widget(block, chunks[2]);
    }
}

/// Border colour for `pane`: accented when it owns the keyboard.
fn border(state: &State, pane: DashboardPane) -> Color {
    if state.focus == Focus::Page && state.dashboard.focus == pane {
        ASCENT
    } else {
        ASCENT_BIS
    }
}
