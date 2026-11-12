//! The window's keyboard shortcuts: each a thing the person can do with a click (the strip's own
//! words for it, `does`), on a key chord they can change in Settings. The catalog holds the
//! defaults; the person's settings hold only what they changed (a chord, or none).

mod catalog;

pub(super) use catalog::{Shortcut, every};
#[cfg(any(target_os = "macos", test))]
pub(super) use catalog::{groups, of_page};

use std::collections::BTreeMap;

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
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "only macOS's menus show the chords")
    )]
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

/// `shortcut`'s chord, given what the person changed: theirs where they set one (none: they took
/// it off), else the catalog's.
fn chord_of(changed: &BTreeMap<String, Option<String>>, shortcut: &Shortcut) -> Option<Chord> {
    match changed.get(shortcut.id) {
        Some(Some(keys)) => Chord::parse(keys),
        Some(None) => None,
        None => Chord::parse(shortcut.keys),
    }
}

/// Each shortcut's chord now: the person's where they set one (none: they took it off), else
/// the catalog's. Ids the catalog no longer has are passed over.
pub(super) fn resolved(
    changed: &BTreeMap<String, Option<String>>,
) -> Vec<(&'static Shortcut, Option<Chord>)> {
    (every().into_iter())
        .map(|shortcut| (shortcut, chord_of(changed, shortcut)))
        .collect()
}

/// Shortcut `id`'s keys as the person has them now (`cmd+t`), none when they took it off or the
/// catalog has no such shortcut.
pub(super) fn keys_of(id: &str) -> Option<String> {
    let shortcut = every().into_iter().find(|shortcut| shortcut.id == id)?;
    let changed = super::settings::now().shortcuts;
    chord_of(&changed, shortcut).map(|chord| chord.text())
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
