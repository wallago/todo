//! Colour palette, the same as praline's.
#![allow(dead_code, reason = "the whole palette, for the widgets you add")]

use ratatui::style::Color;

/// App background.
pub(super) const BACKGROUND: Color = Color::Rgb(25, 26, 27);
/// Accent: focused borders, the selected row, active titles.
pub(super) const ACCENT: Color = Color::Rgb(75, 227, 116);
/// Dim accent: unfocused borders, the selection background, the gauge track.
pub(super) const ACCENT_BIS: Color = Color::Rgb(22, 53, 33);
/// Main text.
pub(super) const PRIMARY: Color = Color::Rgb(207, 233, 212);
/// Muted text: pane titles, list items, key descriptions.
pub(super) const SECONDARY: Color = Color::Rgb(103, 128, 112);
/// Faint decoration: the brackets and arrow in the key reference.
pub(super) const UNACCENT: Color = Color::Rgb(36, 53, 42);
/// Warm highlight.
pub(super) const HIGHLIGHT: Color = Color::Rgb(255, 190, 60);
/// Error.
pub(super) const ERROR: Color = Color::Rgb(235, 87, 87);
