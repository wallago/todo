use ratatui::style::Color;

/// App background.
pub(crate) const BACKGROUND: Color = Color::Rgb(25, 26, 27);
/// Accent: focused borders, the selected row, active titles.
pub(crate) const ASCENT: Color = Color::Rgb(75, 227, 116);
/// Dim accent: unfocused borders, the selection background, the gauge track.
pub(crate) const ASCENT_BIS: Color = Color::Rgb(22, 53, 33);
/// Main text.
pub(crate) const PRIMARY: Color = Color::Rgb(207, 233, 212);
/// Muted text: pane titles, list items, key descriptions.
pub(crate) const SECONDARY: Color = Color::Rgb(103, 128, 112);
/// Faint decoration: the brackets and arrow in the key reference.
pub(crate) const UNASCENT: Color = Color::Rgb(36, 53, 42);
/// Warm highlight.
pub(crate) const HIGHLIGHT: Color = Color::Rgb(255, 190, 60);
/// Error.
pub(crate) const ERROR: Color = Color::Rgb(235, 87, 87);
