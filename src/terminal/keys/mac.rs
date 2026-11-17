//! The keys a Mac terminal has on top of what a terminal's key encoding sends: ⌘←/⌘→ go to the
//! line's start and end, ⌥←/⌥→ by words, ⌘⌫/⌥⌫ delete to the line's start and the word before,
//! ⌘⌦/⌥⌦ to its end and the word after, sent as the readline keys every shell and REPL reads;
//! and Option+Return, Meta Return (ESC CR) only from an Option that is Meta, else Return. The
//! view's own Command keys (⌘K, ⌘↑, ⌘↓, ⌘A) are the view's; the window's go to its menu.

use crate::surface::{Key, KeyAction, KeyCode, Mods};

/// What each key sends with Command, and with Option.
const WITH_COMMAND: [(KeyCode, &[u8]); 4] = [
    (KeyCode::ArrowLeft, b"\x01"),
    (KeyCode::ArrowRight, b"\x05"),
    (KeyCode::Backspace, b"\x15"),
    (KeyCode::Delete, b"\x0b"),
];
const WITH_OPTION: [(KeyCode, &[u8]); 4] = [
    (KeyCode::ArrowLeft, b"\x1bb"),
    (KeyCode::ArrowRight, b"\x1bf"),
    (KeyCode::Backspace, b"\x17"),
    (KeyCode::Delete, b"\x1bd"),
];

fn found(table: &[(KeyCode, &'static [u8])], code: KeyCode) -> Option<&'static [u8]> {
    table
        .iter()
        .find(|(at, _)| *at == code)
        .map(|(_, sent)| *sent)
}

/// What `key` sends as a Mac terminal's own key, `meta` saying whether the Option held is the
/// one that is Meta; none for a key the encoder sends.
pub(in super::super) fn sent(key: &Key, meta: bool) -> Option<&'static [u8]> {
    let mods = key.mods;
    let pressed = matches!(key.action, KeyAction::Press | KeyAction::Repeat);
    let (cmd, alt) = (mods.has(Mods::SUPER), mods.has(Mods::ALT));
    // Control, an input method at work, and Shift with Command or Option go on as they are.
    if !pressed || key.composing || mods.has(Mods::CTRL) || (mods.has(Mods::SHIFT) && (cmd || alt))
    {
        return None;
    }
    if cmd {
        return found(&WITH_COMMAND, key.code);
    }
    if !alt {
        return None;
    }
    if key.code == KeyCode::Enter {
        return Some(if meta { b"\x1b\r" } else { b"\r" });
    }
    found(&WITH_OPTION, key.code)
}

#[cfg(test)]
#[path = "mac_tests.rs"]
mod tests;
