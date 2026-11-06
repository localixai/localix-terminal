//! Tests of `screen`.

use super::*;

/// The rows (blank ones kept to the end of the used ones) and the cursor after `bytes` (fed whole,
/// then a byte at a time: the screen must not depend on how the reads were cut).
fn shown(bytes: &[u8]) -> (Vec<String>, (u16, u16)) {
    let mut whole = Screen::new(40, 120);
    whole.feed(bytes);
    let mut split = Screen::new(40, 120);
    for byte in bytes {
        split.feed(&[*byte]);
    }
    assert_eq!(whole.replay(), split.replay(), "split one byte per read");
    let (text, cursor) = whole.text();
    (text.lines().map(str::to_string).collect(), cursor)
}

#[test]
fn lines_land_on_rows_and_the_cursor_follows() {
    let (rows, cursor) = shown(b"hi\r\nthere");
    assert_eq!(rows, ["hi", "there"]);
    assert_eq!(cursor, (2, 6));
}

#[test]
fn a_carriage_return_redraws_the_row() {
    assert_eq!(shown(b"50%\r100%").0[0], "100%");
}

#[test]
fn only_text_reaches_the_screen() {
    // A title, a clipboard write, a hyperlink, a device control string, a query, a bell, a C1
    // control and NUL: none of them shows, and the text around them does.
    let bytes = "a\x1b]0;title\x07b\x1b]52;c;aGk=\x1b\\c\x1b]8;;http://x\x07d\x1bP1$r0m\x1b\\e\
                 \x1b[6nf\x07g\u{9b}31mh\x00i"
        .as_bytes();
    let (rows, _) = shown(bytes);
    assert_eq!(rows[0], "abcdefg31mhi");
    // Tabs and backspaces move the cursor as a terminal's do.
    let (rows, cursor) = shown(b"ab\x08c\td");
    assert_eq!(rows[0], "ac      d");
    assert_eq!(cursor, (1, 10));
}

#[test]
fn a_replay_carries_the_screen_as_a_terminal_draws_it() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"hi\r\n\x1b[1mthere\x1b[0m and \x1b[31mred\x1b[m\x1b[3;7H");
    let drawn = String::from_utf8(screen.replay()).unwrap();
    // What a terminal shows after it: the same text, styles and cursor.
    let mut again = vt100::Parser::new(40, 120, 0);
    again.process(drawn.as_bytes());
    let (before, after) = (screen.parser.screen(), again.screen());
    assert_eq!(after.contents(), before.contents());
    assert_eq!(after.cursor_position(), (2, 6));
    assert!(after.cell(1, 0).unwrap().bold());
    assert_eq!(after.cell(1, 10).unwrap().fgcolor(), vt100::Color::Idx(1));
    // It draws, and asks nothing: no string sequence, no query, no title.
    assert!(!drawn.contains("\x1b]") && !drawn.contains("\x1bP") && !drawn.contains("[6n"));
}

#[test]
fn a_resized_screen_keeps_its_text_and_reports_its_size() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"hello");
    screen.resize(10, 50);
    assert_eq!(screen.size(), (10, 50));
    assert_eq!(screen.text().0, "hello");
}

#[test]
fn a_title_is_kept_aside_as_words() {
    let mut screen = Screen::new(40, 120);
    assert_eq!(screen.title(), None);
    // Ended by BEL, then by `ESC \`, read a byte at a time: the spinner in front is left off.
    screen.feed(b"\x1b]0;\xe2\x9c\xb3 Fix the cart\x07a");
    assert_eq!(screen.title(), Some("Fix the cart"));
    for byte in "\x1b]2;\u{280b} Fix the cart button\x1b\\".as_bytes() {
        screen.feed(&[*byte]);
    }
    assert_eq!(screen.title(), Some("Fix the cart button"));
    // Other OSCs, a broken one and one with no words leave it.
    screen.feed(b"\x1b]52;c;aGk=\x07\x1b]0;half\x1b[m\x1b]2;\xe2\x9c\xb3 \x07");
    assert_eq!(screen.title(), Some("Fix the cart button"));
    assert_eq!(screen.text().0, "a");
}
