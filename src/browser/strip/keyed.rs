//! What a shortcut does that no button of the strip does (`key …` of shortcuts.rs), read from
//! its words.

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
    pub(super) fn parse(words: &str) -> Option<Self> {
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
            words => match words.strip_prefix("new ") {
                Some(kind) => Self::NewKind(crate::kind::named(kind)?.about().id),
                None => {
                    let at = words.strip_prefix("tab ")?.parse().ok();
                    Self::Tab(at.filter(|at| (1..=8).contains(at))?)
                }
            },
        })
    }
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
