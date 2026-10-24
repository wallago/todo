use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, HighlightSpacing, List, Padding},
};

use crate::{
    app::Opt,
    prelude::OptId,
    ui::{prelude::*, state::mode::DashboardPane},
};

/// Draws selectable option list.
pub(super) fn render_options(state: &mut State, frame: &mut Frame, chunk: Rect) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("options", SECONDARY).bold(),
        Span::raw(" "),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .padding(Padding::new(1, 1, 1, 0))
        .border_style(super::border(state, DashboardPane::Options));
    let mut options = Vec::new();
    push_tree(&state.app.options, None, 0, &mut options);
    let highligh_style = if state
        .dashboard
        .options
        .selected()
        .and_then(|id| state.app.options.get(id))
        .is_some_and(|opt| opt.conflicted)
    {
        Style::new().fg(ERROR).bg(ASCENT_BIS).bold()
    } else {
        Style::new().fg(ASCENT).bg(ASCENT_BIS).bold()
    };
    let list = List::new(options)
        .block(block)
        .style(SECONDARY)
        .highlight_style(highligh_style)
        .highlight_symbol(Span::styled("▌ ", ASCENT))
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(list, chunk, &mut state.dashboard.options);
}

/// Pushes the options hanging off `parent`, each followed by its own
/// children, indented one step per level.
fn push_tree(opts: &[Opt], parent: Option<OptId>, depth: usize, lines: &mut Vec<Line<'static>>) {
    for opt in opts.iter().filter(|opt| opt.id.def().parent == parent) {
        let mark = if opt.checked {
            Span::styled("▬ ", ASCENT)
        } else if opt.conflicted {
            Span::styled("✕ ", ERROR)
        } else {
            Span::raw("  ")
        };
        let indent = Span::raw("  ".repeat(depth));
        let name = Span::raw(<&'static str>::from(opt.id));
        lines.push(Line::from(vec![indent, mark, name]));
        push_tree(opts, Some(opt.id), depth + 1, lines);
    }
}
