//! The terminal core as the session reads it: text and cursor after output, text to look in, the
//! modes a program set, its size, the other screen, history held to its limits, and what a shell
//! said of its commands.

use super::{Content, Core, HISTORY_LINES, Step};
use crate::surface::CellPixels;

fn core(rows: u16, columns: u16) -> Core {
    match Core::new(rows, columns) {
        Ok(core) => core,
        Err(why) => panic!("a core of {rows}×{columns}: {why}"),
    }
}

#[test]
fn text_and_cursor_follow_what_was_written() {
    let mut core = core(5, 20);
    core.feed(b"hello\r\nworld");
    assert_eq!(core.text(), "hello\nworld");
    assert_eq!(core.cursor(), (2, 6));
}

#[test]
fn trailing_spaces_and_blank_rows_at_the_end_are_left_out() {
    let mut core = core(6, 20);
    core.feed(b"\r\nab   \x1b[5;1H \x1b[3;1H");
    assert_eq!(core.text(), "\nab");
    assert_eq!(core.cursor(), (3, 1));
}

#[test]
fn a_moved_cursor_and_an_erase_are_drawn_as_a_terminal_draws_them() {
    let mut core = core(4, 20);
    core.feed(b"first\r\nsecond\x1b[1;3H\x1b[K!\x1b[2;1H\x1b[2K");
    assert_eq!(core.text(), "fi!");
    assert_eq!(core.cursor(), (2, 1));
}

#[test]
fn line_drawing_reads_as_the_box_it_draws() {
    let mut core = core(4, 20);
    core.feed(b"\x1b(0lqqk\r\nx  x\r\nmqqj\x1b(B ok");
    assert_eq!(core.text(), "┌──┐\n│  │\n└──┘ ok");
}

#[test]
fn a_wide_character_is_one_character_in_two_columns() {
    let mut core = core(3, 20);
    core.feed("日本!".as_bytes());
    assert_eq!(core.text(), "日本!");
    assert_eq!(core.cursor(), (1, 6));
}

#[test]
fn a_wide_character_that_wraps_reads_once_on_the_row_it_is_drawn_on() {
    // One column is left on the second row for a character two wide: it goes to the third, the
    // column it left is blank, and nothing of another row is read into it.
    let mut core = core(4, 5);
    core.feed("xy\r\nabcd日e".as_bytes());
    assert_eq!(core.text(), "xy\nabcd\n日e");
    assert_eq!(core.search_text(), "xy\nabcd日e");
    assert_eq!(core.cursor(), (3, 4));
}

#[test]
fn joined_and_combined_characters_read_as_they_were_written() {
    let mut core = core(3, 20);
    let written = "e\u{301} a\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}b";
    core.feed(written.as_bytes());
    assert_eq!(core.text(), written);
    assert_eq!(core.search_text(), written);
}

#[test]
fn a_line_longer_than_the_row_wraps_onto_the_next() {
    let mut core = core(3, 5);
    core.feed(b"abcdefg");
    assert_eq!(core.text(), "abcde\nfg");
    assert_eq!(core.cursor(), (2, 3));
}

#[test]
fn resizing_changes_the_grid_and_keeps_the_text() {
    let mut core = core(10, 40);
    core.feed(b"one\r\ntwo");
    assert!(core.resize(5, 20, CellPixels::default()).is_ok());
    assert_eq!(core.size(), (5, 20));
    assert_eq!(core.text(), "one\ntwo");
    assert!(core.resize(0, 20, CellPixels::default()).is_err());
    assert_eq!(core.size(), (5, 20));
}

#[test]
fn the_other_screen_is_shown_while_a_program_draws_in_it_and_left_after() {
    let mut core = core(4, 20);
    core.feed(b"shell$ ");
    core.feed(b"\x1b[?1049h\x1b[H\x1b[2Jfull screen");
    assert!(core.alternate());
    assert_eq!(core.text(), "full screen");
    core.feed(b"\x1b[?1049l");
    assert!(!core.alternate());
    assert_eq!(core.text(), "shell$");
}

#[test]
fn rows_that_scroll_off_the_top_are_kept_as_history() {
    let mut core = core(3, 10);
    core.feed(b"1\r\n2\r\n3\r\n4\r\n5");
    assert_eq!(core.text(), "3\n4\n5");
    assert_eq!(core.history(), 2);
}

#[test]
fn history_is_held_to_its_limit_however_much_is_written() {
    let mut core = core(40, 120);
    let line = format!("{:>118}\r\n", "x");
    for _ in 0..15 {
        core.feed(line.repeat(1000).as_bytes());
    }
    // The core lets rows go a page at a time, so it keeps within a page of the limit.
    let kept = core.history();
    let page = 1000;
    assert!(kept.abs_diff(HISTORY_LINES) < page, "{kept} rows");
}

#[test]
fn a_grid_of_no_rows_or_no_columns_is_refused() {
    assert!(Core::new(0, 80).is_err());
    assert!(Core::new(24, 0).is_err());
}

#[test]
fn many_cores_come_and_go() {
    for _ in 0..200 {
        let mut core = core(40, 120);
        core.feed(b"\x1b[31mred\x1b[0m");
        assert_eq!(core.text(), "red");
    }
}

#[test]
fn a_core_moves_to_another_thread() {
    let mut core = core(3, 10);
    core.feed(b"here");
    let moved = std::thread::Builder::new()
        .name("core-test".into())
        .spawn(move || core.text())
        .unwrap()
        .join();
    assert_eq!(moved.ok().as_deref(), Some("here"));
}

#[test]
fn text_to_look_in_joins_a_line_that_wrapped() {
    let mut core = core(4, 5);
    core.feed(b"abc  de\r\nfg   \r\n");
    assert_eq!(core.text(), "abc\nde\nfg");
    assert_eq!(core.search_text(), "abc  de\nfg");
}

#[test]
fn the_modes_a_program_set_are_read() {
    let mut core = core(4, 20);
    assert!(core.cursor_visible());
    assert!(!(core.application_cursor() || core.bracketed_paste() || core.mouse_tracking()));
    assert!(!core.synchronized());
    core.feed(b"\x1b[?1h\x1b[?2004h\x1b[?1000h\x1b[?25l\x1b[?2026h");
    assert!(core.application_cursor() && core.bracketed_paste() && core.mouse_tracking());
    assert!(core.synchronized() && !core.cursor_visible());
}

#[test]
fn the_steps_a_shell_reports_are_heard_and_divide_the_cells() {
    let mut core = core(4, 20);
    assert_eq!(core.step(), None);
    core.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    assert_eq!(core.step(), Some(Step::Input));
    assert!(core.at_prompt());
    core.feed(b"ls\r\n\x1b]133;C\x07out\r\n");
    assert_eq!(core.step(), Some(Step::Output));
    assert!(!core.at_prompt());
    core.feed(b"\x1b]133;D;2\x07");
    assert_eq!(core.step(), Some(Step::Ended(Some(2))));
    assert_eq!(core.content(0, 0), Some(Content::Prompt));
    assert_eq!(core.content(0, 2), Some(Content::Input));
    assert_eq!(core.content(1, 0), Some(Content::Output));
    assert_eq!(core.content(9, 0), None, "off the grid");
    assert_eq!(core.text(), "$ ls\nout");
}

/// A character of several codepoints takes the columns Unicode gives it, as the programs that
/// draw beside it count them: an emoji joined by ZWJ is one of two columns, not one per part,
/// and a heart chosen to be an emoji (VS16) is two. A full reset keeps it so.
#[test]
fn a_character_of_several_codepoints_takes_the_columns_unicode_gives_it() {
    let mut core = core(5, 20);
    core.feed("\u{1F469}\u{200D}\u{1F4BB}x".as_bytes());
    assert_eq!(core.cursor(), (1, 4), "a joined emoji then x");
    core.feed("\r\n\u{2764}\u{FE0F}x".as_bytes());
    assert_eq!(core.cursor(), (2, 4), "an emoji heart then x");
    core.feed("\x1bc\u{1F469}\u{200D}\u{1F4BB}x".as_bytes());
    assert_eq!(core.cursor(), (1, 4), "after a full reset");
    // A program may still ask for the widths of old, and has them until it lets go.
    core.feed("\x1b[?2027l\r\n\u{1F469}\u{200D}\u{1F4BB}x".as_bytes());
    assert_eq!(core.cursor(), (2, 6), "widths of old, asked for");
}
