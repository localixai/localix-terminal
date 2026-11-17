//! Tests of `mac`: the keys a Mac terminal has, and the bytes each sends.

use super::sent;
use crate::surface::{Key, KeyAction, KeyCode, Mods};

fn key(code: KeyCode, mods: u16) -> Key {
    Key {
        code,
        mods: Mods(mods),
        ..Key::default()
    }
}

#[test]
fn command_moves_to_the_lines_ends_and_deletes_to_them() {
    let cmd = Mods::SUPER;
    assert_eq!(
        sent(&key(KeyCode::ArrowLeft, cmd), false),
        Some(&b"\x01"[..])
    );
    assert_eq!(
        sent(&key(KeyCode::ArrowRight, cmd), false),
        Some(&b"\x05"[..])
    );
    assert_eq!(
        sent(&key(KeyCode::Backspace, cmd), false),
        Some(&b"\x15"[..])
    );
    assert_eq!(sent(&key(KeyCode::Delete, cmd), false), Some(&b"\x0b"[..]));
    // With Option too, Command's still.
    let both = Mods::SUPER | Mods::ALT;
    assert_eq!(
        sent(&key(KeyCode::ArrowLeft, both), true),
        Some(&b"\x01"[..])
    );
}

#[test]
fn option_moves_and_deletes_by_words_whichever_option_is_meta() {
    for meta in [false, true] {
        let alt = Mods::ALT;
        assert_eq!(
            sent(&key(KeyCode::ArrowLeft, alt), meta),
            Some(&b"\x1bb"[..])
        );
        assert_eq!(
            sent(&key(KeyCode::ArrowRight, alt), meta),
            Some(&b"\x1bf"[..])
        );
        assert_eq!(
            sent(&key(KeyCode::Backspace, alt), meta),
            Some(&b"\x17"[..])
        );
        assert_eq!(sent(&key(KeyCode::Delete, alt), meta), Some(&b"\x1bd"[..]));
    }
}

#[test]
fn option_return_is_meta_return_only_from_an_option_that_is_meta() {
    let alt = Mods::ALT;
    assert_eq!(sent(&key(KeyCode::Enter, alt), true), Some(&b"\x1b\r"[..]));
    assert_eq!(sent(&key(KeyCode::Enter, alt), false), Some(&b"\r"[..]));
}

#[test]
fn other_keys_go_on_to_the_encoder() {
    // Control, Shift beside Command or Option, a key composing, a release, and a plain key.
    for (code, mods) in [
        (KeyCode::ArrowLeft, Mods::SUPER | Mods::CTRL),
        (KeyCode::ArrowLeft, Mods::SUPER | Mods::SHIFT),
        (KeyCode::ArrowLeft, Mods::ALT | Mods::SHIFT),
        (KeyCode::ArrowLeft, 0),
        (KeyCode::Letter(b'k'), Mods::SUPER),
        (KeyCode::Enter, Mods::SHIFT),
    ] {
        assert_eq!(sent(&key(code, mods), true), None, "{code:?} {mods}");
    }
    let composing = Key {
        composing: true,
        ..key(KeyCode::ArrowLeft, Mods::ALT)
    };
    assert_eq!(sent(&composing, true), None);
    let released = Key {
        action: KeyAction::Release,
        ..key(KeyCode::ArrowLeft, Mods::SUPER)
    };
    assert_eq!(sent(&released, true), None);
}
