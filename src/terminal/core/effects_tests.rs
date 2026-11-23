//! Tests of `effects`: the core answers a program's queries itself, once each and only so much
//! a read, with the terminal's own name; and the program's colours and links reach it.

use std::fmt::Write as _;

use super::super::{Core, Render};
use super::REPLIES_MOST;
use crate::surface::{CellPixels, Frame};

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
fn the_terminal_answers_its_terminfo_name_as_programs_are_told_it() {
    let mut core = core(5, 20);
    // XTGETTCAP of "TN", its name hex-encoded: answered valid (1), the name hex-encoded too.
    let name = (super::super::super::TERM.bytes()).fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02X}");
        hex
    });
    assert_eq!(
        answered(&mut core, b"\x1bP+q544E\x1b\\"),
        format!("\x1bP1+r544E={name}\x1b\\")
    );
}

#[test]
fn the_grid_is_answered_in_characters_either_way_it_is_asked() {
    let mut core = core(5, 20);
    assert_eq!(answered(&mut core, b"\x1b[18t"), "\x1b[8;5;20t");
    assert_eq!(answered(&mut core, b"\x1b[19t"), "\x1b[9;5;20t");
}

#[test]
fn the_size_in_pixels_is_answered_from_the_cell_the_view_drew() {
    let mut core = core(5, 20);
    let cell = CellPixels {
        width: 9,
        height: 18,
    };
    core.resize(6, 30, cell).unwrap();
    assert_eq!(core.cell_pixels(), cell);
    assert_eq!(answered(&mut core, b"\x1b[18t"), "\x1b[8;6;30t");
    assert_eq!(answered(&mut core, b"\x1b[16t"), "\x1b[6;18;9t");
    assert_eq!(answered(&mut core, b"\x1b[14t"), "\x1b[4;108;270t");
    // A cell of another size on the same grid: a font's size, a screen of another scale.
    let twice = CellPixels {
        width: 18,
        height: 36,
    };
    core.resize(6, 30, twice).unwrap();
    assert_eq!(answered(&mut core, b"\x1b[16t"), "\x1b[6;36;18t");
}

#[test]
fn with_no_view_the_size_in_pixels_is_answered_as_zero() {
    let mut core = core(5, 20);
    assert_eq!(answered(&mut core, b"\x1b[16t"), "\x1b[6;0;0t");
    assert_eq!(answered(&mut core, b"\x1b[14t"), "\x1b[4;0;0t");
    assert_eq!(answered(&mut core, b"\x1b[18t"), "\x1b[8;5;20t");
}
