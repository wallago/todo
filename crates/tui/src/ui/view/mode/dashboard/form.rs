use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding},
};

use crate::{
    prelude::*,
    ui::{
        prelude::*,
        state::mode::DashboardPane,
        view::{render_button, render_input},
    },
};

/// Draws form fields.
pub(super) fn render_form(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("form", SECONDARY).bold(),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .padding(Padding::new(1, 1, 1, 0))
        .border_style(super::border(state, DashboardPane::Form));
    frame.render_widget(block, chunk);
    let chunks = Layout::new(
        Direction::Vertical,
        [
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(6),
        ],
    )
    .horizontal_margin(2)
    .vertical_margin(1)
    .split(chunk);
    {
        let form = state.focus == Focus::Page && state.dashboard.focus == DashboardPane::Form;
        let fields = [
            (Field::Owner, "owner"),
            (Field::Name, "app"),
            (Field::Desc, "desc"),
        ];
        for (&rect, (field, title)) in chunks.iter().zip(fields) {
            let focused = form && state.dashboard.row.field() == Some(field);
            let editing = state.dashboard.editing.as_ref().filter(|_| focused);
            let error = editing.and_then(|input| field.validate(input.value()).err());
            let value = field.get(&state.app);
            render_input(frame, rect, title, value, focused, editing, error);
        }
        let focused = form && state.dashboard.row == FormRow::Dest;
        render_button(
            frame,
            chunks[3],
            "dest",
            &state.app.dest.to_string_lossy(),
            focused,
            None,
        );
    }
}
