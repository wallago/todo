/// Dashboard pane focus and list cursor.
mod dashboard;
/// Details pane focus, file cursor and preview scroll.
mod details;

pub(crate) use dashboard::*;
pub(crate) use details::*;

/// Which mode is currently showing.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Default,
    Eq,
    strum::EnumIter,
    strum::IntoStaticStr,
    strum::FromRepr,
)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum Mode {
    /// Pick options and watch the repo preview update.
    #[default]
    Dashboard,
    /// Browse the generated files and read each one.
    Details,
    /// App preferences; not built yet.
    Settings,
}

impl Mode {
    /// The mode after this one, or `self` if it's already the last.
    pub(crate) fn next(self) -> Self {
        Self::from_repr(self as usize + 1).unwrap_or(self)
    }

    /// The mode before this one, or `self` if it's already the first.
    pub(crate) fn previous(self) -> Self {
        (self as usize)
            .checked_sub(1)
            .and_then(Self::from_repr)
            .unwrap_or(self)
    }
}
