//! Tests of `screen` with `modes`: the modes a program set, the emulator's and those it does not
//! keep, carried into a replay; a full reset and a resize as a page takes them.

use super::*;

#[test]
fn a_replay_keeps_the_application_keypad_the_program_asked_for() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"vim\x1b=");
    let drawn = String::from_utf8(screen.replay()).unwrap();
    assert!(drawn.contains("\x1b="), "{drawn:?}");
    screen.feed(b"\x1b>");
    let drawn = String::from_utf8(screen.replay()).unwrap();
    assert!(
        drawn.contains("\x1b>") && !drawn.contains("\x1b="),
        "{drawn:?}"
    );
}

#[test]
fn a_full_reset_clears_the_screen_and_every_mode() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"old text\x1b[?1004h\x1b[5 q\x1b=");
    screen.feed(b"\x1bc");
    assert_eq!(screen.text().0, "");
    let drawn = String::from_utf8(screen.replay()).unwrap();
    assert!(
        !drawn.contains("?1004h") && !drawn.contains(" q") && !drawn.contains("\x1b="),
        "{drawn:?}"
    );
}

#[test]
fn a_replay_sets_again_the_modes_the_model_does_not_keep_however_the_reads_cut_them() {
    let bytes = b"\x1b[?1004h\x1b[5 q\x1b]12;#ff8800\x1b\\\x1b[3;20r\x1b[10;4Htext";
    let mut whole = Screen::new(40, 120);
    whole.feed(bytes);
    let mut split = Screen::new(40, 120);
    for byte in bytes {
        split.feed(&[*byte]);
    }
    let drawn = whole.replay();
    assert_eq!(drawn, split.replay());
    let text = String::from_utf8(drawn.clone()).unwrap();
    for mode in [
        "\x1b[?1004h",
        "\x1b[5 q",
        "\x1b]12;#ff8800\x07",
        "\x1b[3;20r",
    ] {
        assert!(text.contains(mode), "{mode:?} in {text:?}");
    }
    // Drawn after the screen, the region leaves the cursor where the program had it.
    let mut again = vt100::Parser::new(40, 120, 0);
    again.process(&drawn);
    assert_eq!(again.screen().cursor_position(), (9, 7));
    assert_eq!(again.screen().contents(), whole.parser.screen().contents());
}

#[test]
fn a_resize_drops_the_scroll_region_and_the_same_size_keeps_it() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"\x1b[3;20r");
    screen.resize(40, 120);
    assert!(
        String::from_utf8(screen.replay())
            .unwrap()
            .contains("\x1b[3;20r")
    );
    screen.resize(30, 120);
    assert!(
        !String::from_utf8(screen.replay())
            .unwrap()
            .contains("3;20r")
    );
}
