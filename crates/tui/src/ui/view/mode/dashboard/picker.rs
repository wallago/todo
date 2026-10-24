use ratatui::{
    Frame,
    layout::Constraint,
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, WidgetRef},
};

use crate::ui::prelude::*;

/// Draws the directory picker over the whole screen while it's open.
pub(super) fn render_picker(state: &State, frame: &mut Frame) {
    let Some(picker) = &state.dashboard.picker else {
        return;
    };
    let area = frame
        .area()
        .centered(Constraint::Percentage(75), Constraint::Percentage(85));
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(picker.cwd().display().to_string(), SECONDARY).bold(),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .padding(Padding::new(1, 1, 1, 0))
        .border_style(ASCENT)
        .bg(BACKGROUND);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    picker.widget().render_ref(inner, frame.buffer_mut());
}
