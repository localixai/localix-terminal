//! Tests of `screen` as the terminal core draws it: the lines a box is drawn with, wide
//! characters, a frame a program holds back, string sequences that show nothing, a program's tab
//! stops and its input modes.

use super::tests::blank;

#[test]
fn a_box_drawn_with_the_line_drawing_set_reads_as_its_lines() {
    // G0 switched to the line drawing set and back, as ncurses draws a box.
    let mut screen = blank(5, 20);
    screen.feed(b"\x1b(0lqqk\r\nx  x\r\nmqqj\x1b(B ok");
    assert_eq!(screen.text().0, "┌──┐\n│  │\n└──┘ ok");
    // G1 set to it and shifted in and out (SO, SI), read a byte at a time.
    let mut shifted = blank(5, 20);
    for byte in b"\x1b)0a\x0eqqq\x0fb" {
        shifted.feed(&[*byte]);
    }
    assert_eq!(shifted.text().0, "a───b");
    // Another escape with an intermediate byte shows nothing, and the text after it shows.
    let mut other = blank(5, 20);
    other.feed(b"\x1b%Gx\x1b(By");
    assert_eq!(other.text().0, "xy");
}

#[test]
fn a_wide_character_takes_two_columns_and_reads_once() {
    let mut screen = blank(5, 20);
    screen.feed("日本!".as_bytes());
    assert_eq!(screen.text(), ("日本!".to_string(), (1, 6)));
    // Joined emoji read as the program wrote them.
    let family = "a\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}b";
    let mut joined = blank(5, 20);
    joined.feed(family.as_bytes());
    assert_eq!(joined.text().0, family);
}

#[test]
fn a_frame_held_back_is_read_as_it_was_until_the_program_lets_it_go() {
    let bytes = b"ready\x1b[?2026h\x1b[H\x1b[2Jhalf a fr";
    for split in [false, true] {
        let mut screen = blank(5, 20);
        if split {
            for byte in bytes {
                screen.feed(&[*byte]);
            }
        } else {
            screen.feed(bytes);
        }
        assert_eq!(
            screen.text(),
            ("ready".to_string(), (1, 6)),
            "split {split}"
        );
        assert_eq!(screen.search_text(), "ready");
        // A program that asks is told where the cursor is now.
        assert_eq!(screen.cursor(), (1, 10));
        screen.feed(b"ame\x1b[?2026l");
        assert_eq!(screen.text().0, "half a frame");
    }
    // A hold that ends and starts again in one read holds the frame between.
    let mut screen = blank(5, 20);
    screen.feed(b"\x1b[?2026hone\x1b[?2026l\x1b[?2026h\x1b[Htwo");
    assert_eq!(screen.text().0, "one");
    // A resize lets the frame go.
    screen.resize(6, 20);
    assert_eq!(screen.text().0, "two");
}

#[test]
fn string_sequences_show_nothing() {
    // A link, a clipboard write, a title and a folder: the link's text shows, nothing else does.
    let mut screen = blank(5, 40);
    screen.feed(
        b"\x1b]8;;https://x.test\x07link\x1b]8;;\x07 \x1b]52;c;aGk=\x07\x1b]0;t\x07\
          \x1b]7;file:///etc\x07\x1b]4;1;rgb:00/00/00\x07ok",
    );
    assert_eq!(screen.text().0, "link ok");
}

#[test]
fn a_tab_stop_the_program_set_is_kept() {
    // The defaults cleared, one set at column 5 (HTS): a tab goes there.
    let mut screen = blank(5, 20);
    screen.feed(b"\x1b[3g\x1b[5G\x1bH\r\tX");
    assert_eq!(screen.text(), ("    X".to_string(), (1, 6)));
}

#[test]
fn the_input_modes_are_the_cores() {
    let mut screen = blank(5, 20);
    assert!(!screen.application_cursor() && !screen.bracketed_paste());
    screen.feed(b"\x1b[?1h\x1b[?2004h");
    assert!(screen.application_cursor() && screen.bracketed_paste());
    screen.feed(b"\x1b[?1l\x1b[?2004l");
    assert!(!screen.application_cursor() && !screen.bracketed_paste());
}
