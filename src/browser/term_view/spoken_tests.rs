//! What a screen reader is told of a terminal: nothing at the first look, the new output as it
//! comes (scrolled up or not), never the echo of what the person typed, never the same twice,
//! and a sentence in place of a flood.

use super::{Lines, MORE, Spoken};
use crate::surface::{Frame, Row, RowId};

fn screen(lines: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
    all.resize(6, String::new());
    all
}

#[test]
fn the_first_look_is_silent_and_the_same_screen_says_nothing() {
    let mut spoken = Spoken::default();
    assert_eq!(spoken.heard(&screen(&["$ "])), None);
    assert_eq!(spoken.heard(&screen(&["$ "])), None);
}

#[test]
fn new_lines_are_read_as_they_come() {
    let mut spoken = Spoken::default();
    spoken.heard(&screen(&["$ make"]));
    let said = spoken.heard(&screen(&["$ make", "built 3 files", "$ "]));
    assert_eq!(said.as_deref(), Some("built 3 files\n$"));
}

#[test]
fn output_that_scrolls_the_screen_reads_only_what_is_new() {
    let mut spoken = Spoken::default();
    let before: Vec<String> = (1..=6).map(|n| format!("line {n}")).collect();
    spoken.heard(&before);
    let after: Vec<String> = (3..=8).map(|n| format!("line {n}")).collect();
    assert_eq!(spoken.heard(&after).as_deref(), Some("line 7\nline 8"));
}

#[test]
fn the_echo_of_what_was_typed_is_not_read_back() {
    let mut spoken = Spoken::default();
    spoken.heard(&screen(&["$ "]));
    spoken.typed("ls");
    assert_eq!(spoken.heard(&screen(&["$ ls"])), None);
    let said = spoken.heard(&screen(&["$ ls", "notes.txt", "$ "]));
    assert_eq!(said.as_deref(), Some("notes.txt\n$"));
}

#[test]
fn what_was_typed_before_the_first_look_holds_nothing_back() {
    let mut spoken = Spoken::default();
    spoken.typed("ab");
    assert_eq!(spoken.heard(&screen(&["$ ab"])), None);
    let said = spoken.heard(&screen(&["$ ab", "ab"]));
    assert_eq!(said.as_deref(), Some("ab"));
}

#[test]
fn a_line_changed_in_place_is_read_whole() {
    let mut spoken = Spoken::default();
    spoken.heard(&screen(&["copying 10%"]));
    let said = spoken.heard(&screen(&["copying 55%"]));
    assert_eq!(said.as_deref(), Some("copying 55%"));
}

#[test]
fn a_flood_is_cut_short_with_a_sentence() {
    let mut spoken = Spoken::default();
    let empty = vec![String::new(); 40];
    spoken.heard(&empty);
    let full: Vec<String> = (1..=40).map(|n| format!("row {n}")).collect();
    let said = spoken.heard(&full).unwrap();
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.len(), 21, "{said}");
    assert_eq!(lines[19], "row 20");
    assert_eq!(lines[20], MORE);
}

#[test]
fn a_grid_of_another_size_is_a_first_look_again() {
    let mut spoken = Spoken::default();
    spoken.heard(&screen(&["$ "]));
    assert_eq!(spoken.heard(&["$ ".to_string(), "x".to_string()]), None);
}

fn row(id: u128, y: u16, text: &str, dirty: bool) -> Row {
    Row {
        id: RowId(id),
        y,
        dirty,
        text: if dirty { text.into() } else { String::new() },
        ..Row::default()
    }
}

#[test]
fn the_rows_a_frame_did_not_change_keep_their_text() {
    let mut lines = Lines::default();
    let first = Frame {
        full: true,
        rows: vec![row(1, 0, "first line", true), row(2, 1, "$ ", true)],
        ..Frame::default()
    };
    lines.keep(&first);
    let next = Frame {
        rows: vec![row(1, 0, "", false), row(2, 1, "$ ls", true)],
        ..Frame::default()
    };
    lines.keep(&next);
    assert_eq!(lines.of(&next), ["first line", "$ ls"]);
    // A row gone out of view is let go of.
    let scrolled = Frame {
        rows: vec![row(2, 0, "", false), row(3, 1, "notes.txt", true)],
        ..Frame::default()
    };
    lines.keep(&scrolled);
    assert_eq!(lines.of(&scrolled), ["$ ls", "notes.txt"]);
    assert_eq!(lines.0.len(), 2);
}
