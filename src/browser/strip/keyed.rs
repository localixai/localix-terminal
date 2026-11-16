//! What a shortcut does that no button of the strip does (`key …` of shortcuts.rs), read from
//! its words.

use crate::layout::geometry::Side;

/// What a shortcut does that has no button of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::browser) enum Keyed {
    /// The project's tab of this place, from 1; the last.
    Tab(u8),
    /// The project switcher's menu, under its button.
    Projects,
    /// A folder picked to open as a project.
    OpenFolder,
    /// The project shown closes: its tabs.
    CloseProject,
    /// The window splits, its tab shown beside another of its project's, or joins again.
    Split,
    /// A new terminal on this side of the pane in use, in its folder.
    SplitNew(Side),
    /// The keys go to the pane on this side of the one in use.
    Toward(Side),
    /// The keys go to the next pane of the tab shown (`true`), or the one before.
    Pane(bool),
    /// The divider beside the pane in use moves towards this side, a long way when `true`.
    Resize(Side, bool),
    /// The pane in use alone over its tab's part, or its tab drawn whole again.
    Zoom,
    /// The tab shown made even.
    Equalize,
    /// The pane in use goes out of its tab into one of its own.
    PaneOut,
    LastTab,
    Next,
    Previous,
    NewWindow,
    NewPage,
    NewTerminal,
    NewBoard,
    /// The project's tab of a kind that draws itself, by the kind's name.
    NewKind(&'static str),
    /// The tab shown closes.
    Close,
    /// The tab closed last opens again.
    Reopen,
    /// The side's search takes the keyboard.
    Find,
    /// Find in what is shown (a terminal's output): the find row opens, or goes a match on or
    /// back.
    FindText(Finding),
    /// The address field takes the keyboard.
    Address,
    /// The page size menu, under its button.
    Size,
    Settings,
}

impl Keyed {
    pub(in crate::browser) fn parse(words: &str) -> Option<Self> {
        Some(match words {
            "tab last" => Self::LastTab,
            "split" => Self::Split,
            "next" => Self::Next,
            "previous" => Self::Previous,
            "new window" => Self::NewWindow,
            "new page" => Self::NewPage,
            "new terminal" => Self::NewTerminal,
            "new board" => Self::NewBoard,
            "close" => Self::Close,
            "reopen" => Self::Reopen,
            "find" => Self::Find,
            "find text" => Self::FindText(Finding::Open),
            "find next" => Self::FindText(Finding::Next),
            "find previous" => Self::FindText(Finding::Previous),
            "address" => Self::Address,
            "size" => Self::Size,
            "settings" => Self::Settings,
            "projects" => Self::Projects,
            "open folder" => Self::OpenFolder,
            "close project" => Self::CloseProject,
            "equalize" => Self::Equalize,
            "zoom" => Self::Zoom,
            "pane out" => Self::PaneOut,
            "pane next" => Self::Pane(true),
            "pane previous" => Self::Pane(false),
            words => match (Self::sided(words), words.strip_prefix("new ")) {
                (Some(sided), _) => sided,
                (None, Some(kind)) => Self::NewKind(crate::kind::named(kind)?.about().id),
                (None, None) => {
                    let at = words.strip_prefix("tab ")?.parse().ok();
                    Self::Tab(at.filter(|at| (1..=8).contains(at))?)
                }
            },
        })
    }

    /// What a shortcut that names a side does: `split right`, `pane up`, `resize far left`.
    fn sided(words: &str) -> Option<Self> {
        let (verb, way) = words.rsplit_once(' ')?;
        let side = side(way)?;
        Some(match verb {
            "split" => Self::SplitNew(side),
            "pane" => Self::Toward(side),
            "resize" => Self::Resize(side, false),
            "resize far" => Self::Resize(side, true),
            _ => return None,
        })
    }
}

/// The side a shortcut's words name: `left`, `right`, `up` or `down`.
fn side(word: &str) -> Option<Side> {
    Some(match word {
        "left" => Side::Left,
        "right" => Side::Right,
        "up" => Side::Up,
        "down" => Side::Down,
        _ => return None,
    })
}

/// A step of Find in what is shown: the find row opens (Find…), or goes to the next match or
/// the one before (Find Next, Find Previous).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::browser) enum Finding {
    Open,
    Next,
    Previous,
}

impl Finding {
    /// The step as the page's `findText` takes it.
    pub(in crate::browser) fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}
