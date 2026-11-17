//! Tests of `screen`.

use super::super::core::{Content, Step};
use super::*;

/// A blank screen of `rows` by `columns`.
pub(super) fn blank(rows: u16, columns: u16) -> Screen {
    Screen::new(rows, columns).unwrap()
}

/// The rows (blank ones kept to the end of the used ones) and the cursor after `bytes` (fed whole,
/// then a byte at a time: the screen must not depend on how the reads were cut).
fn shown(bytes: &[u8]) -> (Vec<String>, (u16, u16)) {
    let mut whole = blank(40, 120);
    whole.feed(bytes);
    let mut split = blank(40, 120);
    for byte in bytes {
        split.feed(&[*byte]);
    }
    let (text, cursor) = whole.text();
    assert_eq!(
        (&text, cursor),
        (&split.text().0, split.cursor()),
        "split one byte per read"
    );
    assert_eq!(
        whole.search_text(),
        split.search_text(),
        "split one byte per read"
    );
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
fn a_resized_screen_keeps_its_text_and_reports_its_size() {
    let mut screen = blank(40, 120);
    screen.feed(b"hello");
    screen.resize(10, 50);
    assert_eq!(screen.size(), (10, 50));
    assert_eq!(screen.text().0, "hello");
    // No rows or no columns is not a size.
    screen.resize(0, 50);
    assert_eq!(screen.size(), (10, 50));
}

#[test]
fn a_resize_wraps_the_lines_again_to_the_new_width() {
    let mut screen = blank(5, 10);
    screen.feed(b"abcdefghijklmno\r\n$ ");
    assert_eq!(screen.text().0, "abcdefghij\nklmno\n$");
    screen.resize(5, 20);
    assert_eq!(screen.text().0, "abcdefghijklmno\n$");
    screen.resize(5, 5);
    assert_eq!(screen.text().0, "abcde\nfghij\nklmno\n$");
    assert_eq!(screen.search_text(), "abcdefghijklmno\n$");
}

#[test]
fn a_title_is_kept_aside_as_words() {
    let mut screen = blank(40, 120);
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
fn a_string_left_open_ends_at_the_next_escape_and_the_screen_goes_on() {
    // A title never ended, then a prompt: the escape that follows begins a sequence of its own.
    let mut screen = blank(40, 120);
    screen.feed(b"a\x1b]0;half a title\x1b[31mb\x1b[mc");
    assert_eq!(screen.text().0, "abc");
    // A cancelled string, then text.
    screen.feed(b"\x1b]0;lost\x18d\x1b]0;lost\x1ae");
    assert_eq!(screen.text().0, "abcde");
    assert_eq!(screen.title(), None);
}

#[test]
fn a_text_that_wraps_at_the_panes_edge_is_found_whole() {
    let mut screen = blank(5, 10);
    screen.feed(b"abcdefghijklmno\r\nxy   z");
    // The snapshot keeps the screen's own rows.
    assert_eq!(screen.text().0, "abcdefghij\nklmno\nxy   z");
    let found = screen.search_text();
    assert_eq!(found, "abcdefghijklmno\nxy   z");
    assert!(found.contains("hijkl") && found.contains("y   z"));
    // A row that ends on its own ends the line, its trailing spaces gone.
    let mut plain = blank(5, 10);
    plain.feed(b"ab   \r\ncd");
    assert_eq!(plain.search_text(), "ab\ncd");
}

#[test]
fn a_shells_marks_are_counted_and_never_drawn() {
    let mut screen = blank(5, 40);
    // A mark split across two reads, ended either way a string ends.
    screen.feed(b"$ \x1b]133;C\x07out\r\n\x1b]133;D");
    assert!(screen.marks().running().is_some());
    screen.feed(b";3\x1b\\$ ");
    let (count, ended) = screen.marks().ended();
    assert_eq!((count, ended.unwrap().code), (1, Some(3)));
    assert_eq!(screen.text().0, "$ out\n$");
    // The core is told them too, and reads its cells by them.
    assert_eq!(screen.core.step(), Some(Step::Ended(Some(3))));
    assert_eq!(screen.core.content(0, 2), Some(Content::Output));
}

#[test]
fn what_a_command_left_on_is_put_right_when_the_shell_says_it_ended() {
    // A program that died with the mouse reported and the cursor hidden: the shell's prompt
    // would take clicks as typing.
    let mut screen = blank(5, 40);
    assert!(!screen.feed(b"\x1b]133;C\x07\x1b[?1002h\x1b[?1006h\x1b[?25ltui"));
    assert!(screen.feed(b"\r\nKilled\r\n\x1b]133;D;137\x07$ "));
    assert!(!screen.core.mouse_tracking() && screen.core.cursor_visible());
    // A command that left nothing on, and output with no command's end in it: nothing is added.
    assert!(!screen.feed(b"\x1b]133;C\x07ok\x1b]133;D;0\x07$ "));
    assert!(!screen.feed(b"\x1b[?1000hplain"));
}

#[test]
fn a_program_the_shell_started_at_once_keeps_what_it_turned_on() {
    // A line typed ahead: the next command starts in the same breath the last one ended.
    let mut screen = blank(5, 40);
    screen.feed(b"\x1b]133;C\x07");
    assert!(!screen.feed(b"\x1b]133;D;0\x07\x1b]133;C\x07\x1b[?1000h"));
    assert!(screen.core.mouse_tracking());
}

/// A report of the program status protocol, as a program writes it, ended by `end`.
fn status(body: &str, end: &str) -> Vec<u8> {
    format!("\x1b]7501;{body}{end}").into_bytes()
}

#[test]
fn a_status_report_is_kept_for_the_terminal_and_never_shows_or_titles() {
    let mut screen = blank(40, 120);
    screen.feed(&status("state=working:app=claude-code", "\x07"));
    screen.feed(b"a");
    screen.feed(&status("state=working:id=a1", "\x1b\\"));
    screen.feed(b"b");
    let rows = screen.status().rows();
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["", "a1"]);
    assert_eq!(screen.text().0, "ab");
    assert_eq!(screen.title(), None);
    assert!(screen.notes().waiting().is_none());
}

#[test]
fn a_long_report_is_read_whole_however_the_reads_cut_it() {
    // A message of 2,000 bytes is a report of near 2,700: past what a title is read from.
    let msg = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "approve Bash: ".to_string() + &"x".repeat(2000),
    );
    let bytes = status(
        &format!("state=blocked:kind=permission:msg={msg}"),
        "\x1b\\",
    );
    assert!(bytes.len() > 2500);
    let mut screen = blank(40, 120);
    let (first, rest) = bytes.split_at(1000);
    screen.feed(first);
    assert!(screen.status().rows().is_empty(), "not before its end");
    screen.feed(rest);
    let rows = screen.status().rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].msg.as_ref().map(String::len), Some(2014));
}

#[test]
fn a_report_past_the_most_is_dropped_never_read_cut_short() {
    let mut screen = blank(40, 120);
    let long = format!("state=working:app=claude-code:{}", "z=1:".repeat(1100));
    screen.feed(&status(&long, "\x07"));
    assert!(screen.status().rows().is_empty());
    // What follows is read as it would be.
    screen.feed(&status("state=done", "\x07"));
    assert_eq!(screen.status().rows().len(), 1);
}

#[test]
fn each_query_is_owed_an_answer_once() {
    let mut screen = blank(40, 120);
    screen.feed(b"\x1b]7501;?\x1b\\\x1b]75");
    screen.feed(b"01;?\x07");
    assert_eq!(screen.status_asked(), 2);
    assert_eq!(screen.status_asked(), 0, "answered, it is owed no more");
}

#[test]
fn a_reset_takes_every_record() {
    let mut screen = blank(40, 120);
    screen.feed(&status("state=done", "\x07"));
    screen.feed(b"\x1b[?1049h");
    assert_eq!(
        screen.status().rows().len(),
        1,
        "the alternate screen keeps them"
    );
    screen.feed(b"\x1bc");
    assert!(screen.status().rows().is_empty());
}
