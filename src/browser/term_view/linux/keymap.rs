//! A GDK key event as the terminal is told it: the key by its place on the keyboard (the
//! evdev code under the event's hardware code, read as a W3C `code`), and the modifiers held,
//! with the side of each that only its right key holds (GDK says what is held, not which).

use crate::surface::{KeyCode, Mods};

/// GDK's modifier bits (`GdkModifierType`): Shift, Caps Lock, Control, Alt (Mod1), Num Lock
/// (Mod2), Super (Mod4 and its own bit).
const SHIFT: u32 = 1 << 0;
const LOCK: u32 = 1 << 1;
const CONTROL: u32 = 1 << 2;
const ALT: u32 = 1 << 3;
const NUM: u32 = 1 << 4;
const MOD4: u32 = 1 << 6;
const SUPER: u32 = 1 << 26;

/// The modifiers `state` holds, with the right side of each held only on the right (`right`:
/// the right keys down now, as `Mods`' right bits).
pub(super) fn mods(state: u32, right: u16) -> Mods {
    let mut mods = 0;
    for (gdk, ours) in [
        (SHIFT, Mods::SHIFT),
        (CONTROL, Mods::CTRL),
        (ALT, Mods::ALT),
        (SUPER | MOD4, Mods::SUPER),
        (LOCK, Mods::CAPS_LOCK),
        (NUM, Mods::NUM_LOCK),
    ] {
        if state & gdk != 0 {
            mods |= ours;
        }
    }
    let sides = [
        (Mods::SHIFT, Mods::SHIFT_RIGHT),
        (Mods::CTRL, Mods::CTRL_RIGHT),
        (Mods::ALT, Mods::ALT_RIGHT),
        (Mods::SUPER, Mods::SUPER_RIGHT),
    ];
    for (held, on_right) in sides {
        if mods & held != 0 && right & on_right != 0 {
            mods |= on_right;
        }
    }
    Mods(mods)
}

/// The right side's bit a modifier key holds while it is down, and whether it is a right one:
/// the left keys clear it, the right ones set it.
pub(super) fn side(code: KeyCode) -> Option<(u16, bool)> {
    const SIDES: [(KeyCode, u16, bool); 8] = [
        (KeyCode::ShiftLeft, Mods::SHIFT_RIGHT, false),
        (KeyCode::ShiftRight, Mods::SHIFT_RIGHT, true),
        (KeyCode::ControlLeft, Mods::CTRL_RIGHT, false),
        (KeyCode::ControlRight, Mods::CTRL_RIGHT, true),
        (KeyCode::AltLeft, Mods::ALT_RIGHT, false),
        (KeyCode::AltRight, Mods::ALT_RIGHT, true),
        (KeyCode::MetaLeft, Mods::SUPER_RIGHT, false),
        (KeyCode::MetaRight, Mods::SUPER_RIGHT, true),
    ];
    (SIDES.iter())
        .find(|(at, ..)| *at == code)
        .map(|(_, bit, right)| (*bit, *right))
}

/// The key at the place GDK's hardware code `hardware` names (X and Wayland alike: the evdev
/// code plus 8).
pub(super) fn code(hardware: u16) -> KeyCode {
    const LETTERS: [(u16, u8); 26] = [
        (30, b'a'),
        (48, b'b'),
        (46, b'c'),
        (32, b'd'),
        (18, b'e'),
        (33, b'f'),
        (34, b'g'),
        (35, b'h'),
        (23, b'i'),
        (36, b'j'),
        (37, b'k'),
        (38, b'l'),
        (50, b'm'),
        (49, b'n'),
        (24, b'o'),
        (25, b'p'),
        (16, b'q'),
        (19, b'r'),
        (31, b's'),
        (20, b't'),
        (22, b'u'),
        (47, b'v'),
        (17, b'w'),
        (45, b'x'),
        (21, b'y'),
        (44, b'z'),
    ];
    const PAD: [u16; 10] = [82, 79, 80, 81, 75, 76, 77, 71, 72, 73];
    let evdev = hardware.saturating_sub(8);
    if let Some((_, letter)) = LETTERS.iter().find(|(at, _)| *at == evdev) {
        return KeyCode::Letter(*letter);
    }
    let digit = |n: u16| u8::try_from(n).ok();
    match evdev {
        2..=10 => digit(evdev - 1).map_or(KeyCode::Unidentified, KeyCode::Digit),
        11 => KeyCode::Digit(0),
        59..=68 => digit(evdev - 58).map_or(KeyCode::Unidentified, KeyCode::F),
        87 | 88 => digit(evdev - 76).map_or(KeyCode::Unidentified, KeyCode::F),
        183..=194 => digit(evdev - 170).map_or(KeyCode::Unidentified, KeyCode::F),
        _ => match PAD.iter().position(|at| *at == evdev) {
            Some(n) => u8::try_from(n).map_or(KeyCode::Unidentified, KeyCode::Numpad),
            None => named(evdev),
        },
    }
}

fn named(evdev: u16) -> KeyCode {
    match evdev {
        1 => KeyCode::Escape,
        12 => KeyCode::Minus,
        13 => KeyCode::Equal,
        14 => KeyCode::Backspace,
        15 => KeyCode::Tab,
        26 => KeyCode::BracketLeft,
        27 => KeyCode::BracketRight,
        28 => KeyCode::Enter,
        29 => KeyCode::ControlLeft,
        39 => KeyCode::Semicolon,
        40 => KeyCode::Quote,
        41 => KeyCode::Backquote,
        42 => KeyCode::ShiftLeft,
        43 => KeyCode::Backslash,
        51 => KeyCode::Comma,
        52 => KeyCode::Period,
        53 => KeyCode::Slash,
        54 => KeyCode::ShiftRight,
        55 => KeyCode::NumpadMultiply,
        56 => KeyCode::AltLeft,
        57 => KeyCode::Space,
        58 => KeyCode::CapsLock,
        74 => KeyCode::NumpadSubtract,
        78 => KeyCode::NumpadAdd,
        83 => KeyCode::NumpadDecimal,
        86 => KeyCode::IntlBackslash,
        96 => KeyCode::NumpadEnter,
        97 => KeyCode::ControlRight,
        98 => KeyCode::NumpadDivide,
        100 => KeyCode::AltRight,
        102 => KeyCode::Home,
        103 => KeyCode::ArrowUp,
        104 => KeyCode::PageUp,
        105 => KeyCode::ArrowLeft,
        106 => KeyCode::ArrowRight,
        107 => KeyCode::End,
        108 => KeyCode::ArrowDown,
        109 => KeyCode::PageDown,
        110 => KeyCode::Insert,
        111 => KeyCode::Delete,
        117 => KeyCode::NumpadEqual,
        125 => KeyCode::MetaLeft,
        126 => KeyCode::MetaRight,
        127 => KeyCode::ContextMenu,
        _ => KeyCode::Unidentified,
    }
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
