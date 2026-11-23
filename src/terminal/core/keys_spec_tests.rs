//! Keys against the published tables, built as the view builds them (the text the input method
//! commits, none for a key it reads as a command; Shift used up where it made the text): the C0
//! keys with every modifier, Control on each ASCII key, the cursor keys in both modes, the
//! editing keys, the function keys and the modifier parameter. Where the core chose one of two
//! readings the tables allow, the test pins its choice and says so.

use crate::surface::{Key, KeyCode, Mods, OptionMeta};
use crate::terminal::Core;

const S: u16 = Mods::SHIFT;
const C: u16 = Mods::CTRL;
const A: u16 = Mods::ALT;

/// What `code` with `mods` sends, typed as the view types it: `text` the committed text, Shift
/// used up when there is text, Option as Meta on both sides.
fn sends(core: &mut Core, code: KeyCode, mods: u16, text: &str, unshifted: Option<char>) -> String {
    let key = Key {
        code,
        mods: Mods(mods),
        consumed: Mods(if text.is_empty() { 0 } else { mods & S }),
        text: text.to_string(),
        unshifted,
        ..Key::default()
    };
    String::from_utf8(core.encode_key(&key, OptionMeta::Both)).unwrap()
}

/// The C0 keys from the keyboard protocol's legacy table, where the core sends what it lists:
/// (key, modifiers, bytes). Return, Tab and Escape with other modifiers are the keys themselves
/// (below), and Shift+Enter is ours (a line feed).
#[test]
fn the_c0_keys_with_each_modifier() {
    let mut core = Core::new(4, 20).unwrap();
    let rows: [(KeyCode, u16, &str); 17] = [
        (KeyCode::Enter, 0, "\r"),
        (KeyCode::Enter, A, "\x1b\r"),
        (KeyCode::Escape, 0, "\x1b"),
        (KeyCode::Escape, A, "\x1b\x1b"),
        (KeyCode::Backspace, 0, "\x7f"),
        (KeyCode::Backspace, C, "\x08"),
        (KeyCode::Backspace, A, "\x1b\x7f"),
        (KeyCode::Backspace, S, "\x7f"),
        (KeyCode::Backspace, A | S, "\x1b\x7f"),
        (KeyCode::Backspace, C | A, "\x1b\x08"),
        (KeyCode::Tab, 0, "\t"),
        (KeyCode::Tab, A, "\x1b\t"),
        (KeyCode::Tab, S, "\x1b[Z"),
        (KeyCode::Space, 0, " "),
        (KeyCode::Space, C, "\0"),
        (KeyCode::Space, A, "\x1b "),
        (KeyCode::Space, C | A, "\x1b\0"),
    ];
    for (code, mods, want) in rows {
        let space = code == KeyCode::Space;
        let text = if space && mods & C == 0 { " " } else { "" };
        let got = sends(&mut core, code, mods, text, space.then_some(' '));
        assert_eq!(got, want, "{code:?} with {mods:#x}");
    }
    let shift_space = sends(&mut core, KeyCode::Space, S, " ", Some(' '));
    assert_eq!(shift_space, " ");
}

/// Control, Shift and Option on Return, Tab and Escape send the key itself (Option as Meta adds
/// its ESC), as terminals send them to a program that reads plain bytes; a program that turned
/// on modifyOtherKeys is told them apart, as it asked.
#[test]
fn return_tab_and_escape_with_modifiers_are_the_keys_themselves() {
    let mut core = Core::new(4, 20).unwrap();
    let keys = [
        (KeyCode::Enter, C, "\r"),
        (KeyCode::Enter, C | S, "\r"),
        (KeyCode::Enter, A | S, "\x1b\r"),
        (KeyCode::Enter, C | A, "\x1b\r"),
        (KeyCode::Tab, C, "\t"),
        (KeyCode::Escape, C, "\x1b"),
        (KeyCode::Escape, S, "\x1b"),
        (KeyCode::Escape, C | A, "\x1b\x1b"),
    ];
    for (code, mods, want) in keys {
        assert_eq!(
            sends(&mut core, code, mods, "", None),
            want,
            "{code:?} {mods:#x}"
        );
    }
    core.feed(b"\x1b[>4;2m");
    let told = sends(&mut core, KeyCode::Enter, C, "", None);
    assert_eq!(told, "\x1b[27;5;13~", "asked for by modifyOtherKeys");
}

/// Control on each ASCII key the table maps, as the view hands it over: no text, the key's own
/// character unshifted.
#[test]
fn control_on_each_ascii_key() {
    let mut core = Core::new(4, 20).unwrap();
    for letter in b'a'..=b'z' {
        let got = sends(
            &mut core,
            KeyCode::Letter(letter),
            C,
            "",
            Some(char::from(letter)),
        );
        assert_eq!(
            got.as_bytes(),
            [letter - b'a' + 1],
            "Ctrl+{}",
            char::from(letter)
        );
    }
    let keys: [(KeyCode, char, &str); 9] = [
        (KeyCode::BracketLeft, '[', "\x1b"),
        (KeyCode::Backslash, '\\', "\x1c"),
        (KeyCode::BracketRight, ']', "\x1d"),
        (KeyCode::Slash, '/', "\x1f"),
        (KeyCode::Digit(2), '2', "\0"),
        (KeyCode::Digit(3), '3', "\x1b"),
        (KeyCode::Digit(6), '6', "\x1e"),
        (KeyCode::Digit(7), '7', "\x1f"),
        (KeyCode::Digit(8), '8', "\x7f"),
    ];
    for (code, unshifted, want) in keys {
        assert_eq!(
            sends(&mut core, code, C, "", Some(unshifted)),
            want,
            "Ctrl+{unshifted}"
        );
    }
    // Once a program asks for the keyboard protocol, Ctrl+[ is told apart from Escape and
    // Ctrl+I from Tab.
    core.feed(b"\x1b[>1u");
    let escape = sends(&mut core, KeyCode::BracketLeft, C, "", Some('['));
    assert_eq!(escape, "\x1b[91;5u");
    let tab = sends(&mut core, KeyCode::Letter(b'i'), C, "", Some('i'));
    assert_eq!(tab, "\x1b[105;5u");
}

/// Control with Shift on a key whose shifted character has a control code, as the view reads
/// it (by the shifted character): the same byte as the table's.
#[test]
fn control_with_shift_by_the_shifted_character() {
    let mut core = Core::new(4, 20).unwrap();
    let keys: [(KeyCode, &str, char, &str); 4] = [
        (KeyCode::Space, " ", ' ', "\0"),
        (KeyCode::Minus, "_", '-', "\x1f"),
        (KeyCode::Digit(6), "^", '6', "\x1e"),
        (KeyCode::Slash, "?", '/', "\x7f"),
    ];
    for (code, text, unshifted, want) in keys {
        assert_eq!(
            sends(&mut core, code, C | S, text, Some(unshifted)),
            want,
            "Ctrl+{text}"
        );
    }
    // A letter with Control and Shift is the letter's control byte; Ctrl+@ is NUL; with Option
    // as Meta, ESC before it.
    let i = sends(&mut core, KeyCode::Letter(b'i'), C | S, "I", Some('i'));
    assert_eq!(i, "\t");
    let a = sends(&mut core, KeyCode::Letter(b'a'), C | S, "A", Some('a'));
    assert_eq!(a, "\x01");
    let at = sends(&mut core, KeyCode::Digit(2), C | S, "@", Some('2'));
    assert_eq!(at, "\0");
    let meta = sends(&mut core, KeyCode::Letter(b'a'), C | S | A, "A", Some('a'));
    assert_eq!(meta, "\x1b\x01");
    // Under the keyboard protocol it is told with both modifiers, as the protocol's table has.
    core.feed(b"\x1b[>1u");
    let told = sends(&mut core, KeyCode::Letter(b'i'), C | S, "I", Some('i'));
    assert_eq!(told, "\x1b[105;6u");
}

#[test]
fn cursor_keys_in_both_modes_and_with_modifiers() {
    let mut core = Core::new(4, 20).unwrap();
    let keys = [
        (KeyCode::ArrowUp, 'A'),
        (KeyCode::ArrowDown, 'B'),
        (KeyCode::ArrowRight, 'C'),
        (KeyCode::ArrowLeft, 'D'),
        (KeyCode::Home, 'H'),
        (KeyCode::End, 'F'),
    ];
    for (code, last) in keys {
        assert_eq!(sends(&mut core, code, 0, "", None), format!("\x1b[{last}"));
        for (mods, n) in [(S, 2), (A, 3), (A | S, 4), (C, 5), (C | S, 6), (C | A, 7)] {
            let want = format!("\x1b[1;{n}{last}");
            assert_eq!(
                sends(&mut core, code, mods, "", None),
                want,
                "{code:?} {mods:#x}"
            );
        }
    }
    core.feed(b"\x1b[?1h");
    for (code, last) in keys {
        assert_eq!(sends(&mut core, code, 0, "", None), format!("\x1bO{last}"));
        let ctrl = sends(&mut core, code, C, "", None);
        assert_eq!(
            ctrl,
            format!("\x1b[1;5{last}"),
            "a modifier leaves SS3 for CSI"
        );
    }
}

#[test]
fn the_editing_keys_and_their_modifiers() {
    let mut core = Core::new(4, 20).unwrap();
    for (code, n) in [
        (KeyCode::Insert, 2),
        (KeyCode::Delete, 3),
        (KeyCode::PageUp, 5),
        (KeyCode::PageDown, 6),
    ] {
        assert_eq!(sends(&mut core, code, 0, "", None), format!("\x1b[{n}~"));
        assert_eq!(sends(&mut core, code, C, "", None), format!("\x1b[{n};5~"));
        assert_eq!(sends(&mut core, code, S, "", None), format!("\x1b[{n};2~"));
    }
}

/// F1 to F12, and with Control: F1, F2 and F4 as `CSI 1;5 P/Q/S`; F3 as `CSI 13;5~`, the
/// core's reading, since `CSI 1;5R` is also a cursor position report.
#[test]
fn the_function_keys_and_control() {
    let mut core = Core::new(4, 20).unwrap();
    let plain = [
        "\x1bOP", "\x1bOQ", "\x1bOR", "\x1bOS", "\x1b[15~", "\x1b[17~", "\x1b[18~", "\x1b[19~",
        "\x1b[20~", "\x1b[21~", "\x1b[23~", "\x1b[24~",
    ];
    for (n, want) in (1..=12).zip(plain) {
        assert_eq!(sends(&mut core, KeyCode::F(n), 0, "", None), want, "F{n}");
    }
    let ctrl = [
        "\x1b[1;5P",
        "\x1b[1;5Q",
        "\x1b[13;5~",
        "\x1b[1;5S",
        "\x1b[15;5~",
    ];
    for (n, want) in (1..=5).zip(ctrl) {
        assert_eq!(
            sends(&mut core, KeyCode::F(n), C, "", None),
            want,
            "Ctrl+F{n}"
        );
    }
    assert_eq!(sends(&mut core, KeyCode::F(5), S, "", None), "\x1b[15;2~");
}

/// A program that turned on modifyOtherKeys (level 2) is told Control on a letter,
/// Tab's letter among them, as `CSI 27;5;<code>~`.
#[test]
fn modify_other_keys_tells_control_letters_apart() {
    let mut core = Core::new(4, 20).unwrap();
    core.feed(b"\x1b[>4;2m");
    let a = sends(&mut core, KeyCode::Letter(b'a'), C, "", Some('a'));
    assert_eq!(a, "\x1b[27;5;97~");
    let i = sends(&mut core, KeyCode::Letter(b'i'), C, "", Some('i'));
    assert_eq!(i, "\x1b[27;5;105~");
}

/// The keyboard protocol's flags as a stack a program pushes, asks about and pops, the other
/// screen with a stack of its own.
#[test]
fn the_keyboard_protocols_stack() {
    let mut core = Core::new(4, 20).unwrap();
    let asked = |core: &mut Core, bytes: &[u8]| {
        core.feed(bytes);
        core.feed(b"\x1b[?u");
        String::from_utf8(core.replies()).unwrap()
    };
    assert_eq!(asked(&mut core, b""), "\x1b[?0u");
    assert_eq!(asked(&mut core, b"\x1b[>1u"), "\x1b[?1u");
    assert_eq!(asked(&mut core, b"\x1b[>8u"), "\x1b[?8u");
    assert_eq!(asked(&mut core, b"\x1b[<u"), "\x1b[?1u");
    assert_eq!(asked(&mut core, b"\x1b[?1049h"), "\x1b[?0u");
    assert_eq!(asked(&mut core, b"\x1b[?1049l"), "\x1b[?1u");
    assert_eq!(asked(&mut core, b"\x1b[<9u"), "\x1b[?0u");
}

/// Under a layout of another script the keys are read by their place: Ctrl+[ (х) an Escape,
/// Ctrl+I (ш) a Tab, Ctrl+Shift+A (Ф) a ^A.
#[test]
fn control_under_another_scripts_layout_by_the_keys_place() {
    let mut core = Core::new(4, 20).unwrap();
    let escape = sends(&mut core, KeyCode::BracketLeft, C, "", Some('х'));
    assert_eq!(escape, "\x1b");
    let tab = sends(&mut core, KeyCode::Letter(b'i'), C, "", Some('ш'));
    assert_eq!(tab, "\t");
    let a = sends(&mut core, KeyCode::Letter(b'a'), C | S, "Ф", Some('ф'));
    assert_eq!(a, "\x01");
    let c = sends(&mut core, KeyCode::Letter(b'c'), C, "", Some('с'));
    assert_eq!(c, "\x03");
}
