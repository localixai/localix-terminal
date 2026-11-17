//! Tests of `effects`: the core answers a program's queries itself, once each and only so much
//! a read, with the terminal's own name; the program's colours and links reach it; and a frame a
//! program holds back is kept as it was when the hold began, for agents and for the view.

use super::super::{Core, Render};
use super::{HOLD_MOST, REPLIES_MOST};
use crate::surface::Frame;

fn core(rows: u16, columns: u16) -> Core {
    match Core::new(rows, columns) {
        Ok(core) => core,
        Err(why) => panic!("a core of {rows}×{columns}: {why}"),
    }
}

/// What the core says back after `bytes`.
fn answered(core: &mut Core, bytes: &[u8]) -> String {
    core.feed(bytes);
    String::from_utf8_lossy(&core.replies()).into_owned()
}

#[test]
fn where_the_cursor_is_is_answered_once() {
    let mut core = core(5, 20);
    assert_eq!(answered(&mut core, b"abc\x1b[6n"), "\x1b[1;4R");
    assert_eq!(core.replies(), b"", "told once");
    // Split across two reads: answered once, when it is whole.
    assert_eq!(answered(&mut core, b"\r\n\x1b["), "");
    assert_eq!(answered(&mut core, b"6n"), "\x1b[2;1R");
}

#[test]
fn the_terminal_names_itself_either_way_it_is_asked() {
    let version = env!("CARGO_PKG_VERSION");
    let name = format!("\x1bP>|llxterm {version}\x1b\\");
    let mut core = core(5, 20);
    assert_eq!(answered(&mut core, b"\x1b[>q"), name);
    assert_eq!(answered(&mut core, b"\x1b[>0q"), name);
}

#[test]
fn the_other_fixed_queries_are_the_cores_to_answer() {
    let mut core = core(5, 20);
    // The device's attributes, and a mode's state (DECRQM): bracketed paste, set.
    assert!(answered(&mut core, b"\x1b[c").starts_with("\x1b[?"));
    core.feed(b"\x1b[?2004h");
    assert_eq!(answered(&mut core, b"\x1b[?2004$p"), "\x1b[?2004;1$y");
    // The text colour the theme gave, asked by OSC 10.
    let mut theme = crate::surface::Palette {
        foreground: crate::surface::Rgb::hex(0x11_22_33),
        background: crate::surface::Rgb::hex(0xff_ff_ff),
        cursor: crate::surface::Rgb::hex(0),
        ansi: [crate::surface::Rgb::default(); 16],
    };
    theme.ansi[1] = crate::surface::Rgb::hex(0xaa_00_00);
    core.set_colors(&theme);
    let told = answered(&mut core, b"\x1b]10;?\x1b\\");
    assert!(told.contains("rgb:1111/2222/3333"), "{told:?}");
}

#[test]
fn a_flood_of_queries_is_answered_only_so_far() {
    let mut core = core(5, 20);
    let told = answered(&mut core, &b"\x1b[6n\x1b[>q".repeat(2000));
    assert!(
        !told.is_empty() && told.len() <= REPLIES_MOST,
        "{}",
        told.len()
    );
}

#[test]
fn a_programs_colours_and_links_reach_the_core() {
    let mut core = core(5, 40);
    core.feed(b"\x1b]4;1;rgb:12/34/56\x07\x1b]8;;https://x.test/a\x07link\x1b]8;;\x07 plain");
    assert_eq!(core.text(), "link plain");
    assert_eq!(core.hyperlink_at(0, 1).as_deref(), Some("https://x.test/a"));
    assert_eq!(core.hyperlink_at(0, 6), None);
    let mut render = Render::new().unwrap();
    let mut frame = Frame::default();
    assert!(render.frame(&core, &mut frame, true));
    assert_eq!(
        frame.colors.palette[1],
        crate::surface::Rgb::hex(0x12_34_56)
    );
}

#[test]
fn a_held_frame_is_kept_as_it_was_when_the_hold_began() {
    let mut core = core(5, 20);
    let drawn = core.drawn();
    *crate::lock(&drawn) = Render::new().ok();
    core.feed(b"ready\x1b[?2026h\x1b[H\x1b[2Jhalf a fr");
    assert!(core.holding());
    assert_eq!(core.held_text(), Some(("ready".to_string(), (1, 6))));
    assert_eq!(core.held_search().as_deref(), Some("ready"));
    // The view's render state was brought to that moment, and is not updated meanwhile.
    let mut frame = Frame::default();
    let finished = crate::lock(&drawn)
        .as_mut()
        .map(|r| r.finish(&mut frame, true));
    assert_eq!(finished, Some(true));
    assert!(frame.rows.first().is_some_and(|row| row.text == "ready"));
    core.feed(b"ame\x1b[?2026l");
    assert!(!core.holding());
    assert_eq!(core.held_text(), None);
    assert_eq!(core.text(), "half a frame");
}

#[test]
fn a_hold_ends_as_the_grid_is_resized_and_past_its_most() {
    let mut core = core(5, 20);
    core.feed(b"\x1b[?2026h");
    assert!(core.holding());
    core.resize(6, 20).unwrap();
    assert!(!core.holding(), "a resize lets go");
    core.feed(b"\x1b[?2026h");
    assert!(!core.let_go_if_long(), "not yet");
    crate::backoff(HOLD_MOST);
    assert!(!core.holding(), "past its most it is not believed");
    assert!(core.let_go_if_long());
    assert!(!core.synchronized(), "and the mode is let go of");
}

#[test]
fn a_core_made_deaf_hears_no_hold_and_reads_no_screen_for_one() {
    let mut core = core(4, 20);
    core.deafen();
    core.feed(b"\x1b[?2026hheld");
    assert!(!core.holding());
    assert_eq!(core.held_text(), None);
}
