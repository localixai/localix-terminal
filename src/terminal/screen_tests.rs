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
fn a_replay_preserves_palette_and_truecolor_foregrounds() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"\x1b[31mR\x1b[38;5;202mI\x1b[38;2;12;180;240mT\x1b[0m");
    let mut again = vt100::Parser::new(40, 120, 0);
    again.process(&screen.replay());
    for (column, expected) in [
        vt100::Color::Idx(1),
        vt100::Color::Idx(202),
        vt100::Color::Rgb(12, 180, 240),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            again
                .screen()
                .cell(0, u16::try_from(column).unwrap())
                .unwrap()
                .fgcolor(),
            expected,
        );
    }
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

#[test]
fn a_replay_keeps_the_alternate_screen_and_the_input_modes_the_program_asked_for() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"shell\x1b[?2004h\x1b[?1049h\x1b[?1000h\x1b[?1h\x1b[Hvim");
    let drawn = String::from_utf8(screen.replay()).unwrap();
    for sequence in ["\x1b[?1049h", "\x1b[?2004h", "\x1b[?1000h", "\x1b[?1h"] {
        assert!(drawn.contains(sequence), "{sequence:?} in {drawn:?}");
    }
    assert!(
        drawn.starts_with("\x1b[?1049h"),
        "the switch comes before the drawing"
    );
    // On the normal screen there is no switch.
    let mut normal = Screen::new(40, 120);
    normal.feed(b"hi\x1b[?2004h");
    let drawn = String::from_utf8(normal.replay()).unwrap();
    assert!(
        drawn.contains("\x1b[?2004h") && !drawn.contains("1049"),
        "{drawn:?}"
    );
}

#[test]
fn a_string_left_open_ends_at_the_next_escape_and_the_screen_goes_on() {
    // A title never ended, then a prompt: the escape that follows begins a sequence of its own.
    let mut screen = Screen::new(40, 120);
    screen.feed(b"a\x1b]0;half a title\x1b[31mb\x1b[mc");
    assert_eq!(screen.text().0, "abc");
    // A cancelled string, then text.
    screen.feed(b"\x1b]0;lost\x18d\x1b]0;lost\x1ae");
    assert_eq!(screen.text().0, "abcde");
    assert_eq!(screen.title(), None);
}

#[test]
fn a_text_that_wraps_at_the_panes_edge_is_found_whole() {
    let mut screen = Screen::new(5, 10);
    screen.feed(b"abcdefghijklmno\r\nxy   z");
    // The snapshot keeps the screen's own rows.
    assert_eq!(screen.text().0, "abcdefghij\nklmno\nxy   z");
    let found = screen.search_text();
    assert_eq!(found, "abcdefghijklmno\nxy   z");
    assert!(found.contains("hijkl") && found.contains("y   z"));
    // A row that ends on its own ends the line, its trailing spaces gone.
    let mut plain = Screen::new(5, 10);
    plain.feed(b"ab   \r\ncd");
    assert_eq!(plain.search_text(), "ab\ncd");
}

#[test]
fn a_shells_marks_are_counted_and_never_drawn() {
    let mut screen = Screen::new(5, 40);
    // A mark split across two reads, ended either way a string ends.
    screen.feed(b"$ \x1b]133;C\x07out\r\n\x1b]133;D");
    assert!(screen.marks().running().is_some());
    screen.feed(b";3\x1b\\$ ");
    let (count, ended) = screen.marks().ended();
    assert_eq!((count, ended.unwrap().code), (1, Some(3)));
    assert_eq!(screen.text().0, "$ out\n$");
}

#[test]
fn what_a_command_left_on_is_put_right_when_the_shell_says_it_ended() {
    // A program that died with the mouse reported and the cursor hidden: the shell's prompt
    // would take clicks as typing.
    let mut screen = Screen::new(5, 40);
    assert!(
        screen
            .feed(b"\x1b]133;C\x07\x1b[?1002h\x1b[?1006h\x1b[?25ltui")
            .is_empty()
    );
    let righted = screen.feed(b"\r\nKilled\r\n\x1b]133;D;137\x07$ ");
    assert_eq!(righted, RIGHTED);
    let drawn = String::from_utf8(screen.replay()).unwrap();
    assert!(
        !drawn.contains("?1002h") && !drawn.contains("?25l"),
        "{drawn:?}"
    );
    // A command that left nothing on, and output with no command's end in it: nothing is added.
    assert!(
        screen
            .feed(b"\x1b]133;C\x07ok\x1b]133;D;0\x07$ ")
            .is_empty()
    );
    assert!(screen.feed(b"\x1b[?1000hplain").is_empty());
}

#[test]
fn a_program_the_shell_started_at_once_keeps_what_it_turned_on() {
    // A line typed ahead: the next command starts in the same breath the last one ended.
    let mut screen = Screen::new(5, 40);
    screen.feed(b"\x1b]133;C\x07");
    let righted = screen.feed(b"\x1b]133;D;0\x07\x1b]133;C\x07\x1b[?1000h");
    assert!(righted.is_empty());
}
