//! Tests of `modes`: the modes a program set that the screen's model does not keep, read from
//! its sequences and written again after a redrawn screen.

use super::Modes;

/// `modes` after each CSI of `sequences` (the bytes after `ESC [`, its final byte last).
fn after(sequences: &[&str]) -> Modes {
    let mut modes = Modes::default();
    for sequence in sequences {
        let (body, end) = sequence.split_at(sequence.len() - 1);
        modes.csi(body.as_bytes(), end.as_bytes()[0]);
    }
    modes
}

/// What a redraw writes after the screen, the cursor at row 5, column 9, as text.
fn replayed(modes: &Modes) -> String {
    String::from_utf8(modes.replayed((5, 9))).unwrap()
}

#[test]
fn focus_reports_are_asked_again_until_turned_off() {
    assert_eq!(replayed(&after(&["?1004h"])), "\x1b[?1004h");
    assert_eq!(replayed(&after(&["?1004h", "?1004l"])), "");
    // One sequence that sets several, the model's own among them.
    assert_eq!(replayed(&after(&["?2004;1004h"])), "\x1b[?1004h");
}

#[test]
fn the_cursors_shape_is_set_again_and_its_default_writes_nothing() {
    assert_eq!(replayed(&after(&["5 q"])), "\x1b[5 q");
    assert_eq!(replayed(&after(&["5 q", "0 q"])), "");
    // With no number, as a terminal reads it: a blinking block.
    assert_eq!(replayed(&after(&[" q"])), "\x1b[1 q");
    // A shape no terminal has is not kept.
    assert_eq!(replayed(&after(&["9 q"])), "");
}

#[test]
fn the_cursors_colour_is_kept_only_when_it_reads_as_a_colour() {
    let mut modes = Modes::default();
    assert!(modes.osc("12;#ff8800"));
    assert_eq!(replayed(&modes), "\x1b]12;#ff8800\x07");
    for colour in ["rgb:ff/88/00", "DarkOrange", "#f80"] {
        assert!(modes.osc(&format!("12;{colour}")));
        assert_eq!(replayed(&modes), format!("\x1b]12;{colour}\x07"));
    }
    // Reset by OSC 112; a query, a sequence, a long value or no value are not colours.
    assert!(modes.osc("112"));
    assert_eq!(replayed(&modes), "");
    let long = format!("12;#{}", "f".repeat(200));
    for said in ["12;?", "12;\x1b[31m", "12;red;blue", "12;", long.as_str()] {
        assert!(modes.osc(said));
        assert_eq!(replayed(&modes), "", "{said:?}");
    }
    // Other strings are not the cursor's.
    assert!(!modes.osc("0;a title") && !modes.osc("133;A") && !modes.osc("1337;x"));
}

#[test]
fn wrapping_reverse_video_and_insert_are_set_again() {
    let modes = after(&["?7l", "?5h", "4h"]);
    assert_eq!(replayed(&modes), "\x1b[?7l\x1b[?5h\x1b[4h");
    assert_eq!(
        replayed(&after(&["?7l", "?7h", "?5h", "?5l", "4h", "4l"])),
        ""
    );
}

#[test]
fn a_scroll_region_is_set_after_the_screen_and_the_cursor_put_back() {
    // The region homes the cursor: it is put back where the screen has it.
    assert_eq!(replayed(&after(&["3;20r"])), "\x1b[3;20r\x1b[5;9H");
    // To the bottom, whatever the height.
    assert_eq!(replayed(&after(&["3r"])), "\x1b[3r\x1b[5;9H");
    // The whole screen, no region, and one that ends before it starts are no region.
    for sequences in [&["3;20r", "r"][..], &["1r"], &["20;3r"], &["3;20r", "1;0r"]] {
        assert_eq!(replayed(&after(sequences)), "", "{sequences:?}");
    }
}

#[test]
fn in_origin_mode_the_cursor_is_put_back_within_the_region() {
    assert_eq!(
        replayed(&after(&["3;20r", "?6h"])),
        "\x1b[3;20r\x1b[?6h\x1b[3;9H"
    );
    assert_eq!(replayed(&after(&["?6h"])), "\x1b[?6h\x1b[5;9H");
}

#[test]
fn a_resize_drops_the_region_as_the_page_does() {
    let mut modes = after(&["3;20r", "?1004h"]);
    modes.resized();
    assert_eq!(replayed(&modes), "\x1b[?1004h");
}

#[test]
fn a_full_reset_clears_every_mode_and_a_soft_one_all_but_the_colour() {
    let mut modes = after(&["?1004h", "5 q", "?7l", "?5h", "4h", "3;20r", "?6h"]);
    assert!(modes.osc("12;#ff8800"));
    let mut soft = modes.clone();
    soft.csi(b"!", b'p');
    assert_eq!(replayed(&soft), "\x1b]12;#ff8800\x07");
    modes.reset();
    assert_eq!(replayed(&modes), "");
}

#[test]
fn other_sequences_are_no_modes() {
    let modes = after(&[
        "31m", "2J", "?1049h", "?25l", ">4;2m", "?1004$p", "6n", "1;2H",
    ]);
    assert_eq!(replayed(&modes), "");
}
