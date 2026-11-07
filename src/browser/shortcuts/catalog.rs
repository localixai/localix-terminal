//! The one catalog of shortcuts, their default chords and page-only actions.

/// A shortcut: what it is called, where Settings lists it, its chord unless the person changed
/// it, and the strip's click it does.
pub(in super::super) struct Shortcut {
    pub id: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub keys: &'static str,
    pub does: &'static str,
}

const fn at(
    id: &'static str,
    title: &'static str,
    group: &'static str,
    keys: &'static str,
    does: &'static str,
) -> Shortcut {
    Shortcut {
        id,
        title,
        group,
        keys,
        does,
    }
}

/// Every shortcut, in the order Settings lists them. The chords follow the system's browsers and
/// editors, so the hand already knows them.
pub(in super::super) const SHORTCUTS: [Shortcut; 29] = [
    at(
        "open-folder",
        "Open Folder\u{2026}",
        "Project",
        "cmd+o",
        "key open folder",
    ),
    at(
        "switch-project",
        "Switch Project\u{2026}",
        "Project",
        "ctrl+cmd+p",
        "key projects",
    ),
    at(
        "close-project",
        "Close Project",
        "Project",
        "ctrl+cmd+w",
        "key close project",
    ),
    // One chord for the eight: its modifiers with a digit show the project's tab of that place.
    at(
        "tab-place",
        "Show a Tab by Its Place",
        "Tabs",
        "cmd+digit",
        "key tab",
    ),
    at("tab-last", "Show Last Tab", "Tabs", "cmd+9", "key tab last"),
    at("tab-next", "Next Tab", "Tabs", "cmd+shift+]", "key next"),
    at(
        "tab-previous",
        "Previous Tab",
        "Tabs",
        "cmd+shift+[",
        "key previous",
    ),
    at(
        "new-window",
        "New Window",
        "Tabs",
        "cmd+n",
        "key new window",
    ),
    at("new-page", "New Page", "Tabs", "cmd+t", "key new page"),
    at(
        "new-terminal",
        "New Terminal",
        "Tabs",
        "cmd+alt+t",
        "key new terminal",
    ),
    at("board", "Board", "Tabs", "cmd+shift+b", "key new board"),
    at("close-tab", "Close Tab", "Tabs", "cmd+w", "key close"),
    at(
        "reopen-tab",
        "Reopen Closed Tab",
        "Tabs",
        "cmd+shift+t",
        "key reopen",
    ),
    at("find-tab", "Search Tabs", "Tabs", "cmd+shift+a", "key find"),
    at("split", "Split View", "Tabs", "cmd+shift+\\", "key split"),
    at("address", "Open Location", "Page", "cmd+l", "key address"),
    at("reload", "Reload Page", "Page", "cmd+r", "reload"),
    at(
        "outside",
        "Open in Default Browser",
        "Page",
        "cmd+shift+o",
        "outside",
    ),
    at("back", "Back", "Page", "cmd+[", "back"),
    at("forward", "Forward", "Page", "cmd+]", "forward"),
    at("page-size", "Page Size", "Page", "cmd+shift+m", "key size"),
    at(
        "comment",
        "Comment on the Page",
        "Page",
        "cmd+shift+c",
        "comment",
    ),
    at("send", "Send Comments", "Page", "cmd+enter", "send"),
    at(
        "sidebar",
        "Show or Hide the Sidebar",
        "Window",
        "ctrl+cmd+s",
        "side",
    ),
    at("history", "History", "Window", "cmd+y", "history"),
    at("events", "Event Center", "Window", "cmd+shift+e", "events"),
    at(
        "collapse",
        "Collapse to the Mini",
        "Window",
        "cmd+m",
        "collapse",
    ),
    at("settings", "Settings…", "LLX Desk", "cmd+,", "key settings"),
    at("quit", "Quit LLX Desk", "LLX Desk", "cmd+q", "quit"),
];

/// What only a web page shown can do: walk its history, load it again, size it, comment on it.
/// Over a terminal, the board or the settings these are off (their menu items greyed).
#[cfg(any(target_os = "macos", test))]
pub(super) const OF_PAGE: [&str; 6] = [
    "reload", "outside", "back", "forward", "key size", "comment",
];

/// Whether what a shortcut `does` needs a web page shown.
#[cfg(any(target_os = "macos", test))]
pub(in super::super) fn of_page(does: &str) -> bool {
    OF_PAGE.contains(&does)
}

/// The groups of the catalog, in its order: Settings lists them so, and each is a menu of the
/// app's menu bar (the first, the app's own).
#[cfg(any(target_os = "macos", test))]
pub(in super::super) fn groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = SHORTCUTS.iter().map(|shortcut| shortcut.group).collect();
    groups.dedup();
    groups
}
