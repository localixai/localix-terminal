//! A GDK key read as the terminal is told it: its place by the evdev code under the hardware
//! code, and the modifiers with the side only the right key holds.

use super::{code, mods, side};
use crate::surface::{KeyCode, Mods};

#[test]
fn the_hardware_code_names_the_place_on_the_keyboard() {
    assert_eq!(
        code(38),
        KeyCode::Letter(b'a'),
        "evdev 30, under X and Wayland's 8 more"
    );
    assert_eq!(code(56), KeyCode::Letter(b'b'));
    assert_eq!(code(10), KeyCode::Digit(1));
    assert_eq!(code(19), KeyCode::Digit(0));
    assert_eq!(code(67), KeyCode::F(1));
    assert_eq!(code(96), KeyCode::F(12));
    assert_eq!(code(90), KeyCode::Numpad(0));
    assert_eq!(code(36), KeyCode::Enter);
    assert_eq!(code(113), KeyCode::ArrowLeft);
    assert_eq!(code(119), KeyCode::Delete);
    assert_eq!(code(108), KeyCode::AltRight);
    assert_eq!(code(0), KeyCode::Unidentified);
}

#[test]
fn modifiers_keep_the_side_only_the_right_key_holds() {
    let (alt, control) = (1 << 3, 1 << 2);
    assert_eq!(mods(alt, 0), Mods(Mods::ALT));
    assert_eq!(
        mods(alt, Mods::ALT_RIGHT),
        Mods(Mods::ALT | Mods::ALT_RIGHT)
    );
    assert_eq!(
        mods(control, Mods::ALT_RIGHT),
        Mods(Mods::CTRL),
        "a side held for nothing"
    );
    assert_eq!(mods(1 << 26, 0), Mods(Mods::SUPER));
    assert_eq!(side(KeyCode::AltRight), Some((Mods::ALT_RIGHT, true)));
    assert_eq!(side(KeyCode::Letter(b'a')), None);
}
