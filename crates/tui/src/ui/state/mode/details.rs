use ratatui::widgets::ListState;

/// Focusable panes on the details, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumIter)]
pub(crate) enum DetailsPane {
    /// Staged file list.
    #[default]
    List,
    /// Current file selected.
    Content,
}

/// Everything the details remembers between visits.
#[derive(Debug, Default)]
pub(crate) struct Details {
    /// Pane that takes the keys while the page has focus.
    pub(crate) focus: DetailsPane,
    /// Cursor in the staged file list.
    pub(crate) files: ListState,
    /// Preview scroll, in rows.
    pub(crate) scroll: u16,
}
