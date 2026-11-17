//! A Mac key event as the terminal is told it: the key by its place on the keyboard (the
//! system's virtual key code read as a W3C `code`), and the modifiers held with the side each was
//! pressed on (the device's own bits beside the system's).

use crate::surface::{KeyCode, Mods};

/// The system's modifier bits (`NSEventModifierFlags`).
const CAPS: usize = 1 << 16;
const SHIFT: usize = 1 << 17;
const CONTROL: usize = 1 << 18;
const OPTION: usize = 1 << 19;
const COMMAND: usize = 1 << 20;

/// The device's bits for each side (`NX_DEVICE…KEYMASK`): left, then right.
const SIDES: [(usize, usize, usize, u16, u16); 4] = [
    (SHIFT, 0x02, 0x04, Mods::SHIFT, Mods::SHIFT_RIGHT),
    (CONTROL, 0x01, 0x2000, Mods::CTRL, Mods::CTRL_RIGHT),
    (OPTION, 0x20, 0x40, Mods::ALT, Mods::ALT_RIGHT),
    (COMMAND, 0x08, 0x10, Mods::SUPER, Mods::SUPER_RIGHT),
];

/// The modifiers `flags` holds: each held one, and its right side when only that one is down.
pub(super) fn mods(flags: usize) -> Mods {
    let mut mods = 0;
    for (system, left, right, held, on_right) in SIDES {
        if flags & system != 0 {
            mods |= held;
            if flags & right != 0 && flags & left == 0 {
                mods |= on_right;
            }
        }
    }
    if flags & CAPS != 0 {
        mods |= Mods::CAPS_LOCK;
    }
    Mods(mods)
}

/// `flags` with Option taken off, as a key is read when Option is Meta: the layout then gives
/// the key's own character, never the one it types with Option.
pub(super) const fn without_option(flags: usize) -> usize {
    flags & !OPTION & !0x20 & !0x40
}

/// What a key made Meta sends after its ESC: the character the layout gives it, but a letter
/// key in a layout of another script gives its Latin letter (so Meta-b moves a word back under
/// a Cyrillic layout too).
pub(super) fn meta_text(code: KeyCode, text: &str, shift: bool) -> String {
    if let KeyCode::Letter(letter) = code
        && !text.is_ascii()
    {
        let letter = if shift {
            letter.to_ascii_uppercase()
        } else {
            letter
        };
        return char::from(letter).to_string();
    }
    text.to_string()
}

/// The key at the place the system's virtual key `code` names (ANSI layout's places).
pub(super) fn code(code: u16) -> KeyCode {
    const LETTERS: [(u16, u8); 26] = [
        (0x00, b'a'),
        (0x0b, b'b'),
        (0x08, b'c'),
        (0x02, b'd'),
        (0x0e, b'e'),
        (0x03, b'f'),
        (0x05, b'g'),
        (0x04, b'h'),
        (0x22, b'i'),
        (0x26, b'j'),
        (0x28, b'k'),
        (0x25, b'l'),
        (0x2e, b'm'),
        (0x2d, b'n'),
        (0x1f, b'o'),
        (0x23, b'p'),
        (0x0c, b'q'),
        (0x0f, b'r'),
        (0x01, b's'),
        (0x11, b't'),
        (0x20, b'u'),
        (0x09, b'v'),
        (0x0d, b'w'),
        (0x07, b'x'),
        (0x10, b'y'),
        (0x06, b'z'),
    ];
    const DIGITS: [u16; 10] = [0x1d, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1a, 0x1c, 0x19];
    const PAD: [u16; 10] = [0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5b, 0x5c];
    const FS: [u16; 20] = [
        0x7a, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6d, 0x67, 0x6f, 0x69, 0x6b, 0x71,
        0x6a, 0x40, 0x4f, 0x50, 0x5a,
    ];
    let index = |list: &[u16]| list.iter().position(|at| *at == code);
    if let Some((_, letter)) = LETTERS.iter().find(|(at, _)| *at == code) {
        return KeyCode::Letter(*letter);
    }
    if let Some(n) = index(&DIGITS).and_then(|n| u8::try_from(n).ok()) {
        return KeyCode::Digit(n);
    }
    if let Some(n) = index(&PAD).and_then(|n| u8::try_from(n).ok()) {
        return KeyCode::Numpad(n);
    }
    if let Some(n) = index(&FS).and_then(|n| u8::try_from(n + 1).ok()) {
        return KeyCode::F(n);
    }
    named(code)
}

fn named(code: u16) -> KeyCode {
    match code {
        0x32 => KeyCode::Backquote,
        0x2a => KeyCode::Backslash,
        0x21 => KeyCode::BracketLeft,
        0x1e => KeyCode::BracketRight,
        0x2b => KeyCode::Comma,
        0x18 => KeyCode::Equal,
        0x1b => KeyCode::Minus,
        0x2f => KeyCode::Period,
        0x27 => KeyCode::Quote,
        0x29 => KeyCode::Semicolon,
        0x2c => KeyCode::Slash,
        0x0a => KeyCode::IntlBackslash,
        0x33 => KeyCode::Backspace,
        0x30 => KeyCode::Tab,
        0x24 => KeyCode::Enter,
        0x31 => KeyCode::Space,
        0x35 => KeyCode::Escape,
        0x75 => KeyCode::Delete,
        0x72 => KeyCode::Insert,
        0x73 => KeyCode::Home,
        0x77 => KeyCode::End,
        0x74 => KeyCode::PageUp,
        0x79 => KeyCode::PageDown,
        0x7e => KeyCode::ArrowUp,
        0x7d => KeyCode::ArrowDown,
        0x7b => KeyCode::ArrowLeft,
        0x7c => KeyCode::ArrowRight,
        0x45 => KeyCode::NumpadAdd,
        0x4e => KeyCode::NumpadSubtract,
        0x43 => KeyCode::NumpadMultiply,
        0x4b => KeyCode::NumpadDivide,
        0x41 => KeyCode::NumpadDecimal,
        0x4c => KeyCode::NumpadEnter,
        0x51 => KeyCode::NumpadEqual,
        0x39 => KeyCode::CapsLock,
        0x38 => KeyCode::ShiftLeft,
        0x3c => KeyCode::ShiftRight,
        0x3b => KeyCode::ControlLeft,
        0x3e => KeyCode::ControlRight,
        0x3a => KeyCode::AltLeft,
        0x3d => KeyCode::AltRight,
        0x37 => KeyCode::MetaLeft,
        0x36 => KeyCode::MetaRight,
        0x6e => KeyCode::ContextMenu,
        _ => KeyCode::Unidentified,
    }
}

/// The modifier a modifier key holds, for a change of modifiers (`flagsChanged:`): whether it
/// is down now.
pub(super) fn held_now(key: KeyCode, flags: usize) -> Option<bool> {
    const HOLDS: [(KeyCode, usize, usize); 9] = [
        (KeyCode::ShiftLeft, SHIFT, 0x02),
        (KeyCode::ShiftRight, SHIFT, 0x04),
        (KeyCode::ControlLeft, CONTROL, 0x01),
        (KeyCode::ControlRight, CONTROL, 0x2000),
        (KeyCode::AltLeft, OPTION, 0x20),
        (KeyCode::AltRight, OPTION, 0x40),
        (KeyCode::MetaLeft, COMMAND, 0x08),
        (KeyCode::MetaRight, COMMAND, 0x10),
        (KeyCode::CapsLock, CAPS, CAPS),
    ];
    let (_, system, side) = HOLDS.iter().find(|(at, ..)| *at == key)?;
    Some(flags & system != 0 && flags & side != 0)
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
