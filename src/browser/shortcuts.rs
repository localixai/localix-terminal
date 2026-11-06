//! The window's keyboard shortcuts: each a thing the person can do with a click (the strip's own
//! words for it, `does`), on a key chord they can change in Settings. The catalog holds the
//! defaults; the person's settings hold only what they changed (a chord, or none).

use std::collections::BTreeMap;

/// A shortcut: what it is called, where Settings lists it, its chord unless the person changed
/// it, and the strip's click it does.
pub(super) struct Shortcut {
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
pub(super) const SHORTCUTS: [Shortcut; 27] = [
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
const OF_PAGE: [&str; 6] = [
    "reload", "outside", "back", "forward", "key size", "comment",
];

/// Whether what a shortcut `does` needs a web page shown.
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(dead_code, reason = "the menu bar that greys them is macOS's")
)]
pub(super) fn of_page(does: &str) -> bool {
    OF_PAGE.contains(&does)
}

/// The groups of the catalog, in its order: Settings lists them so, and each is a menu of the
/// app's menu bar (the first, the app's own).
pub(super) fn groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = SHORTCUTS.iter().map(|shortcut| shortcut.group).collect();
    groups.dedup();
    groups
}

/// A key chord: the modifiers held and the key (a lower-case letter or digit, a symbol, or a
/// named key: `enter`, `tab`, `space`, `escape`, `backspace`, `up`…). `digit` is each of 1 to 8,
/// one chord for a row of them (a tab by its place).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a chord is four modifiers, each held or not, as the system's key events say"
)]
pub(super) struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub key: String,
}

/// The digits a `digit` chord stands for.
pub(super) const PLACES: std::ops::RangeInclusive<u8> = 1..=8;

const NAMED: [&str; 11] = [
    "digit",
    "enter",
    "tab",
    "space",
    "escape",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
];

impl Chord {
    /// `cmd+shift+]` as a chord; none for words that are no chord, or a key with no modifier
    /// but shift (typing, not a shortcut).
    pub(super) fn parse(text: &str) -> Option<Self> {
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            key: String::new(),
        };
        let lower = text.trim().to_ascii_lowercase();
        // The key may itself be `+`: what is after the last `+` that follows a modifier.
        let (mods, key) = match lower.strip_suffix("++") {
            Some(mods) => (mods, "+"),
            None => lower.rsplit_once('+').unwrap_or(("", lower.as_str())),
        };
        for part in mods.split('+').filter(|part| !part.is_empty()) {
            match part {
                "ctrl" | "control" => chord.ctrl = true,
                "alt" | "option" | "opt" => chord.alt = true,
                "shift" => chord.shift = true,
                "cmd" | "command" | "meta" | "super" => chord.cmd = true,
                _ => return None,
            }
        }
        let key = match key {
            "return" => "enter",
            "esc" => "escape",
            other => other,
        };
        let single = key.chars().count() == 1 && key.chars().all(|c| c.is_ascii_graphic());
        if !(single || NAMED.contains(&key)) || !(chord.ctrl || chord.alt || chord.cmd) {
            return None;
        }
        chord.key = key.to_string();
        Some(chord)
    }

    /// As written in the settings: `ctrl+alt+shift+cmd+key`.
    pub(super) fn text(&self) -> String {
        let mut out = String::new();
        for (held, name) in [
            (self.ctrl, "ctrl+"),
            (self.alt, "alt+"),
            (self.shift, "shift+"),
            (self.cmd, "cmd+"),
        ] {
            if held {
                out.push_str(name);
            }
        }
        out.push_str(&self.key);
        out
    }

    /// As the system's menus show it: `⌃⌥⇧⌘T`.
    pub(super) fn shown(&self) -> String {
        let mut out = String::new();
        for (held, mark) in [
            (self.ctrl, '\u{2303}'),
            (self.alt, '\u{2325}'),
            (self.shift, '\u{21e7}'),
            (self.cmd, '\u{2318}'),
        ] {
            if held {
                out.push(mark);
            }
        }
        let key = match self.key.as_str() {
            "enter" => "\u{21a9}".to_string(),
            "tab" => "\u{21e5}".to_string(),
            "space" => "Space".to_string(),
            "escape" => "\u{238b}".to_string(),
            "backspace" => "\u{232b}".to_string(),
            "delete" => "\u{2326}".to_string(),
            "up" => "\u{2191}".to_string(),
            "down" => "\u{2193}".to_string(),
            "left" => "\u{2190}".to_string(),
            "right" => "\u{2192}".to_string(),
            "digit" => format!("{}\u{2026}{}", PLACES.start(), PLACES.end()),
            key => key.to_uppercase(),
        };
        out.push_str(&key);
        out
    }

    /// The key a menu item is given for it, and whether shift goes in the item's modifiers.
    /// A shifted symbol is the symbol shift makes on a US layout (`⇧⌘[` is `{`), as the
    /// system's own menus name it; a shifted letter is the letter, shift in the modifiers.
    pub(super) fn menu_key(&self) -> (String, bool) {
        const SHIFTED: [(char, char); 21] = [
            ('[', '{'),
            (']', '}'),
            (',', '<'),
            ('.', '>'),
            ('/', '?'),
            (';', ':'),
            ('\'', '"'),
            ('-', '_'),
            ('=', '+'),
            ('`', '~'),
            ('\\', '|'),
            ('1', '!'),
            ('2', '@'),
            ('3', '#'),
            ('4', '$'),
            ('5', '%'),
            ('6', '^'),
            ('7', '&'),
            ('8', '*'),
            ('9', '('),
            ('0', ')'),
        ];
        let named = match self.key.as_str() {
            "enter" => Some('\r'),
            "tab" => Some('\t'),
            "space" => Some(' '),
            "escape" => Some('\u{1b}'),
            "backspace" => Some('\u{8}'),
            "delete" => Some('\u{7f}'),
            "up" => Some('\u{f700}'),
            "down" => Some('\u{f701}'),
            "left" => Some('\u{f702}'),
            "right" => Some('\u{f703}'),
            _ => None,
        };
        if let Some(named) = named {
            return (named.to_string(), self.shift);
        }
        let key = self.key.chars().next().unwrap_or(' ');
        match SHIFTED.iter().find(|(plain, _)| *plain == key) {
            Some((_, shifted)) if self.shift => (shifted.to_string(), false),
            _ => (key.to_string(), self.shift),
        }
    }
}

/// Each shortcut's chord now: the person's where they set one (none: they took it off), else
/// the catalog's. Ids the catalog no longer has are passed over.
pub(super) fn resolved(
    changed: &BTreeMap<String, Option<String>>,
) -> Vec<(&'static Shortcut, Option<Chord>)> {
    SHORTCUTS
        .iter()
        .map(|shortcut| {
            let chord = match changed.get(shortcut.id) {
                Some(Some(keys)) => Chord::parse(keys),
                Some(None) => None,
                None => Chord::parse(shortcut.keys),
            };
            (shortcut, chord)
        })
        .collect()
}

impl Chord {
    /// The chords it is: a `digit` chord one for each place, any other itself.
    pub(super) fn each(&self) -> Vec<(Self, Option<u8>)> {
        if self.key != "digit" {
            return vec![(self.clone(), None)];
        }
        (PLACES)
            .map(|place| {
                let key = place.to_string();
                (
                    Self {
                        key,
                        ..self.clone()
                    },
                    Some(place),
                )
            })
            .collect()
    }

    /// Whether some key both chords are.
    pub(super) fn meets(&self, other: &Self) -> bool {
        let theirs = other.each();
        (self.each().iter()).any(|(mine, _)| theirs.iter().any(|(their, _)| their == mine))
    }
}

/// The shortcuts that share a chord with another, by id: only the first of them works.
pub(super) fn clashes(resolved: &[(&'static Shortcut, Option<Chord>)]) -> Vec<&'static str> {
    let mut out = vec![];
    for (at, (shortcut, chord)) in resolved.iter().enumerate() {
        let Some(chord) = chord else { continue };
        let shared = (resolved.iter().enumerate()).any(|(other, (_, theirs))| {
            other != at && theirs.as_ref().is_some_and(|theirs| theirs.meets(chord))
        });
        if shared {
            out.push(shortcut.id);
        }
    }
    out
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;
