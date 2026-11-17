//! Keys as the program receives them: plain text and control keys, cursor keys in the mode the
//! program set, the keyboard protocol once a program pushes its flags, Option as Alt only on the
//! side chosen, and Shift+Enter a line feed until a program asks for the protocol.

use crate::surface::{Key, KeyAction, KeyCode, Mods, OptionMeta};
use crate::terminal::Core;

fn core() -> Core {
    Core::new(4, 20).unwrap()
}

fn key(code: KeyCode, mods: u16, text: &str, unshifted: Option<char>) -> Key {
    Key {
        code,
        mods: Mods(mods),
        text: text.to_string(),
        unshifted,
        ..Key::default()
    }
}

fn sent(core: &mut Core, key: &Key) -> String {
    String::from_utf8(core.encode_key(key, OptionMeta::Off)).unwrap()
}

#[test]
fn text_and_control_keys() {
    let mut core = core();
    assert_eq!(
        sent(&mut core, &key(KeyCode::Letter(b'a'), 0, "a", Some('a'))),
        "a"
    );
    assert_eq!(
        sent(
            &mut core,
            &key(KeyCode::Letter(b'c'), Mods::CTRL, "", Some('c'))
        ),
        "\x03"
    );
    assert_eq!(sent(&mut core, &key(KeyCode::Enter, 0, "\r", None)), "\r");
    assert_eq!(
        sent(&mut core, &key(KeyCode::Backspace, 0, "", None)),
        "\x7f"
    );
    assert_eq!(sent(&mut core, &key(KeyCode::Escape, 0, "", None)), "\x1b");
    assert_eq!(sent(&mut core, &key(KeyCode::F(1), 0, "", None)), "\x1bOP");
    assert_eq!(
        sent(&mut core, &key(KeyCode::ShiftLeft, Mods::SHIFT, "", None)),
        "",
        "a modifier alone"
    );
}

#[test]
fn cursor_keys_follow_the_mode_the_program_set() {
    let mut core = core();
    let up = key(KeyCode::ArrowUp, 0, "", None);
    assert_eq!(sent(&mut core, &up), "\x1b[A");
    core.feed(b"\x1b[?1h");
    assert_eq!(sent(&mut core, &up), "\x1bOA");
    let shifted = key(KeyCode::ArrowLeft, Mods::SHIFT, "", None);
    assert_eq!(sent(&mut core, &shifted), "\x1b[1;2D");
}

#[test]
fn the_keyboard_protocol_once_a_program_asks_for_it() {
    let mut core = core();
    core.feed(b"\x1b[>1u");
    assert_eq!(core.keyboard_flags(), 1);
    assert_eq!(
        sent(&mut core, &key(KeyCode::Escape, 0, "", None)),
        "\x1b[27u"
    );
    assert_eq!(
        sent(
            &mut core,
            &key(KeyCode::Letter(b'c'), Mods::CTRL, "", Some('c'))
        ),
        "\x1b[99;5u"
    );
    core.feed(b"\x1b[<u");
    assert_eq!(core.keyboard_flags(), 0, "popped");
}

#[test]
fn shift_enter_is_a_line_feed_until_the_program_asks_for_the_protocol() {
    let mut core = core();
    let shift_enter = key(KeyCode::Enter, Mods::SHIFT, "\r", None);
    assert_eq!(sent(&mut core, &shift_enter), "\n");
    let right = key(KeyCode::Enter, Mods::SHIFT | Mods::SHIFT_RIGHT, "\r", None);
    assert_eq!(sent(&mut core, &right), "\n", "either Shift");
    core.feed(b"\x1b[>1u");
    assert_eq!(sent(&mut core, &shift_enter), "\x1b[13;2u");
}

/// On Linux Alt is Meta on both sides whatever the setting, as terminals there have it: a key
/// with Alt sends ESC and the character it types.
#[cfg(not(target_os = "macos"))]
#[test]
fn alt_is_meta_on_either_side_on_linux() {
    let mut core = core();
    let left = key(KeyCode::Letter(b'b'), Mods::ALT, "b", Some('b'));
    let right = key(
        KeyCode::Letter(b'b'),
        Mods::ALT | Mods::ALT_RIGHT,
        "b",
        Some('b'),
    );
    for meta in [
        OptionMeta::Off,
        OptionMeta::Left,
        OptionMeta::Right,
        OptionMeta::Both,
    ] {
        for key in [&left, &right] {
            let sent = String::from_utf8(core.encode_key(key, meta)).unwrap();
            assert_eq!(sent, "\x1bb", "{meta:?}");
        }
    }
}

/// A Mac's Option types a character of its own (⌥B: ∫) unless it is the side made Meta.
#[cfg(target_os = "macos")]
#[test]
fn option_is_alt_only_on_the_side_chosen() {
    let mut core = core();
    let left = key(KeyCode::Letter(b'b'), Mods::ALT, "∫", Some('b'));
    let right = key(
        KeyCode::Letter(b'b'),
        Mods::ALT | Mods::ALT_RIGHT,
        "∫",
        Some('b'),
    );
    let with =
        |core: &mut Core, key: &Key, meta| String::from_utf8(core.encode_key(key, meta)).unwrap();
    assert_eq!(with(&mut core, &left, OptionMeta::Left), "\x1bb");
    assert_eq!(
        with(&mut core, &right, OptionMeta::Left),
        "∫",
        "the other side types its character"
    );
    assert_eq!(with(&mut core, &right, OptionMeta::Right), "\x1bb");
    assert_eq!(with(&mut core, &left, OptionMeta::Both), "\x1bb");
    assert_eq!(with(&mut core, &left, OptionMeta::Off), "∫");
    let enter = key(KeyCode::Enter, Mods::ALT, "\r", None);
    assert_eq!(
        with(&mut core, &enter, OptionMeta::Left),
        "\x1b\r",
        "Option+Enter as Meta"
    );
}

#[test]
fn a_release_sends_nothing_unless_asked() {
    let mut core = core();
    let release = Key {
        action: KeyAction::Release,
        ..key(KeyCode::Letter(b'a'), 0, "a", Some('a'))
    };
    assert_eq!(sent(&mut core, &release), "");
}
