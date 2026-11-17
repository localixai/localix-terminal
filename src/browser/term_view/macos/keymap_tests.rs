//! Tests of `keymap`: keys by their place, and the side of each modifier held.

use super::{code, held_now, meta_text, mods, without_option};
use crate::surface::{KeyCode, Mods};

#[test]
fn keys_are_read_by_their_place() {
    assert_eq!(code(0x00), KeyCode::Letter(b'a'));
    assert_eq!(code(0x06), KeyCode::Letter(b'z'));
    assert_eq!(code(0x1d), KeyCode::Digit(0));
    assert_eq!(code(0x19), KeyCode::Digit(9));
    assert_eq!(code(0x52), KeyCode::Numpad(0));
    assert_eq!(code(0x7a), KeyCode::F(1));
    assert_eq!(code(0x6f), KeyCode::F(12));
    assert_eq!(code(0x24), KeyCode::Enter);
    assert_eq!(code(0x33), KeyCode::Backspace);
    assert_eq!(code(0x75), KeyCode::Delete);
    assert_eq!(code(0x7b), KeyCode::ArrowLeft);
    assert_eq!(code(0x3d), KeyCode::AltRight);
    assert_eq!(code(0xff), KeyCode::Unidentified);
}

#[test]
fn each_modifier_says_its_side() {
    // Option on the left (its device bit 0x20), then on the right (0x40), then both.
    let left = (1 << 19) | 0x20;
    let right = (1 << 19) | 0x40;
    assert_eq!(mods(left), Mods(Mods::ALT));
    assert_eq!(mods(right), Mods(Mods::ALT | Mods::ALT_RIGHT));
    assert_eq!(mods(left | right), Mods(Mods::ALT));
    // Shift and Command together, Command on the right.
    let shift_cmd = (1 << 17) | 0x02 | (1 << 20) | 0x10;
    assert_eq!(
        mods(shift_cmd),
        Mods(Mods::SHIFT | Mods::SUPER | Mods::SUPER_RIGHT)
    );
    assert_eq!(mods(without_option(left | (1 << 17))), Mods(Mods::SHIFT));
}

#[test]
fn a_modifier_key_says_whether_it_is_down_now() {
    let right_option = (1 << 19) | 0x40;
    assert_eq!(held_now(KeyCode::AltRight, right_option), Some(true));
    assert_eq!(held_now(KeyCode::AltLeft, right_option), Some(false));
    assert_eq!(held_now(KeyCode::AltRight, 0), Some(false));
    assert_eq!(held_now(KeyCode::Letter(b'a'), right_option), None);
}

#[test]
fn a_letter_made_meta_is_its_latin_letter_in_any_layout() {
    // A Cyrillic layout's и on the B key: Meta-b, as readline binds it.
    assert_eq!(meta_text(KeyCode::Letter(b'b'), "и", false), "b");
    assert_eq!(meta_text(KeyCode::Letter(b'b'), "И", true), "B");
    // A Latin layout's own letter, and what is not a letter, as the layout makes them.
    assert_eq!(meta_text(KeyCode::Letter(b'y'), "z", false), "z");
    assert_eq!(meta_text(KeyCode::Digit(1), "1", false), "1");
    assert_eq!(meta_text(KeyCode::Unidentified, "ö", false), "ö");
}
