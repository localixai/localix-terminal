//! The pointer, focus and pastes as the program asked to hear them: nothing while it asks for
//! no mouse, presses in the X10 and SGR forms, motion only while a button is held in mode 1002
//! and always in 1003, the wheel as buttons 4 and 5; focus only in mode 1004; pastes bracketed
//! in mode 2004, their line breaks returns otherwise, and control bytes that could act made
//! spaces.

use crate::surface::{Button, Geometry, Mouse, MouseAction};
use crate::terminal::Core;

/// A view of 20 columns by 4 rows of 10 by 20 pixel cells, no gutter.
const VIEW: Geometry = Geometry {
    width: 200,
    height: 80,
    cell_width: 10,
    cell_height: 20,
    pad_top: 0,
    pad_bottom: 0,
    pad_left: 0,
    pad_right: 0,
};

/// The pointer over the cell at `column` and `row`, from 0.
fn at(column: f32, row: f32, button: Option<Button>, action: MouseAction) -> Mouse {
    Mouse {
        x: column * 10.0 + 5.0,
        y: row * 20.0 + 5.0,
        button,
        action,
        ..Mouse::default()
    }
}

fn sent(core: &mut Core, mouse: &Mouse, held: bool) -> String {
    String::from_utf8(core.encode_mouse(mouse, &VIEW, held)).unwrap()
}

#[test]
fn nothing_goes_to_a_program_that_asked_for_no_mouse() {
    let mut core = Core::new(4, 20).unwrap();
    assert_eq!(
        sent(
            &mut core,
            &at(1.0, 1.0, Some(Button::Left), MouseAction::Press),
            false
        ),
        ""
    );
}

#[test]
fn presses_in_the_form_the_program_asked() {
    let mut core = Core::new(4, 20).unwrap();
    core.feed(b"\x1b[?1000h");
    let press = at(2.0, 1.0, Some(Button::Left), MouseAction::Press);
    assert_eq!(sent(&mut core, &press, true), "\x1b[M #\"");
    core.feed(b"\x1b[?1006h");
    assert_eq!(sent(&mut core, &press, true), "\x1b[<0;3;2M");
    let release = at(2.0, 1.0, Some(Button::Left), MouseAction::Release);
    assert_eq!(sent(&mut core, &release, false), "\x1b[<0;3;2m");
    let wheel = at(0.0, 0.0, Some(Button::WheelUp), MouseAction::Press);
    assert_eq!(sent(&mut core, &wheel, false), "\x1b[<64;1;1M");
}

#[test]
fn motion_while_dragging_in_1002_and_always_in_1003() {
    let mut core = Core::new(4, 20).unwrap();
    core.feed(b"\x1b[?1002h\x1b[?1006h");
    let moved = at(4.0, 2.0, None, MouseAction::Motion);
    assert_eq!(sent(&mut core, &moved, false), "", "no button held");
    let dragged = at(5.0, 2.0, Some(Button::Left), MouseAction::Motion);
    assert_eq!(sent(&mut core, &dragged, true), "\x1b[<32;6;3M");
    core.feed(b"\x1b[?1003h");
    let anywhere = at(6.0, 3.0, None, MouseAction::Motion);
    assert_eq!(sent(&mut core, &anywhere, false), "\x1b[<35;7;4M");
}

#[test]
fn focus_is_reported_only_while_asked() {
    let mut core = Core::new(4, 20).unwrap();
    assert!(core.focus_report(true).is_empty());
    core.feed(b"\x1b[?1004h");
    assert_eq!(core.focus_report(true), b"\x1b[I");
    assert_eq!(core.focus_report(false), b"\x1b[O");
}

#[test]
fn a_paste_is_bracketed_when_asked_and_made_safe() {
    let mut core = Core::new(4, 20).unwrap();
    assert_eq!(core.paste_bytes("one\ntwo"), b"one\rtwo");
    // Lines copied from Windows or a web page end in CR LF: one Return each, not two.
    assert_eq!(core.paste_bytes("one\r\ntwo\r\n"), b"one\rtwo\r");
    assert_eq!(core.paste_bytes("one\rtwo"), b"one\rtwo");
    core.feed(b"\x1b[?2004h");
    assert_eq!(core.paste_bytes("one\ntwo"), b"\x1b[200~one\ntwo\x1b[201~");
    assert_eq!(
        core.paste_bytes("a\x1b[201~b"),
        b"\x1b[200~a [201~b\x1b[201~",
        "a paste cannot end itself"
    );
}

#[test]
fn the_wheel_on_the_other_screen_as_keys_when_asked() {
    let mut core = Core::new(4, 20).unwrap();
    core.feed(b"\x1b[?1007h");
    assert!(core.alternate_scroll());
}
