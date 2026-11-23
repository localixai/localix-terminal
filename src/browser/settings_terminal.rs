//! A terminal's own settings (Settings, Terminal): which Option key it takes for Meta, whether
//! its code font's ligatures join (on by default; off draws `->` and `!=` as their letters, in
//! fonts that join them), and where a link Command-clicked in it opens.

use serde::{Deserialize, Serialize};

/// Which Option key is Meta in a terminal: with it, Option and a key send ESC and the key, as
/// shells, editors and coding agents read Alt; the other Option types the layout's characters.
/// Neither by default: many layouts (German, French, Nordic, Polish) type @ [ ] { } | \ ~ with
/// either Option, and a person who wants Meta chooses the side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::browser) enum OptionMeta {
    Left,
    Right,
    Both,
    #[default]
    Off,
}

/// Whether a terminal's ligatures join: a code font's standard ligatures and its contextual
/// alternates, both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::browser) enum Ligatures {
    #[default]
    On,
    Off,
}

impl Ligatures {
    pub(in crate::browser) const fn on(self) -> bool {
        matches!(self, Self::On)
    }
}

/// Where a link the person Command-clicks in a terminal opens: they are asked each time (by
/// default), or it opens at once in a tab of the window, or in their default browser.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::browser) enum OpenLinks {
    #[default]
    Ask,
    Tab,
    Browser,
}
