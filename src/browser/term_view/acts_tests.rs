//! What the person's acts mean in any view: the Command keys, text, the wheel in rows, the
//! menu's items, the cell under the pointer, the scrollbar's slider and the view's geometry.

use super::{
    Command, Picked, bar_row, cell_at, command, consumed, geometry, grip, menu, printable,
    wheel_rows,
};
use crate::browser::choice::Entry;
use crate::surface::{Frame, Key, KeyCode, Mods, Scrollbar};

fn key(code: KeyCode, mods: u16) -> Key {
    Key {
        code,
        mods: Mods(Mods::SUPER | mods),
        ..Key::default()
    }
}

#[test]
fn command_keys_are_the_views_or_a_terminals_own() {
    assert_eq!(command(&key(KeyCode::Letter(b'k'), 0)), Command::Clear);
    assert_eq!(command(&key(KeyCode::ArrowUp, 0)), Command::Top);
    assert_eq!(command(&key(KeyCode::ArrowDown, 0)), Command::Bottom);
    assert_eq!(
        command(&key(KeyCode::ArrowLeft, Mods::SHIFT)),
        Command::Send
    );
    assert_eq!(
        command(&key(KeyCode::Letter(b'k'), Mods::SHIFT)),
        Command::Nothing
    );
    assert_eq!(command(&key(KeyCode::Letter(b't'), 0)), Command::Nothing);
}

#[test]
fn text_is_what_prints_and_takes_the_modifiers_that_made_it() {
    assert_eq!(printable("é"), "é");
    assert_eq!(printable("\u{3}"), "");
    let held = Mods(Mods::SHIFT | Mods::CTRL);
    assert_eq!(consumed("A", held, Mods::SHIFT), Mods(Mods::SHIFT));
    assert_eq!(consumed("", held, Mods::SHIFT), Mods(0));
}

#[test]
fn a_trackpads_fine_turns_gather_into_whole_rows() {
    let mut rest = 0.0;
    assert_eq!(wheel_rows(&mut rest, 10.0, true, 16.0), 0);
    assert_eq!(
        wheel_rows(&mut rest, 10.0, true, 16.0),
        1,
        "20 of 16 points: one row"
    );
    assert_eq!(
        wheel_rows(&mut rest, -0.2, false, 16.0),
        -1,
        "a wheel's step is a row at least"
    );
    assert_eq!(wheel_rows(&mut rest, 3.0, false, 16.0), 3);
}

#[test]
fn the_menu_copies_only_a_selection_and_clears_only_the_main_screen() {
    let labels = |entries: &[Entry]| -> Vec<String> {
        (entries.iter())
            .map(|entry| match entry {
                Entry::Item { label, .. } => label.clone(),
                Entry::Line { .. } => "-".into(),
                Entry::Sub { label, .. } => label.clone(),
            })
            .collect()
    };
    let (entries, picks) = menu(true, true);
    assert_eq!(
        labels(&entries),
        ["Copy", "Paste", "-", "Select All", "Clear"]
    );
    assert_eq!(
        picks,
        [
            Picked::Copy,
            Picked::Paste,
            Picked::SelectAll,
            Picked::Clear
        ]
    );
    let (entries, picks) = menu(false, false);
    assert_eq!(labels(&entries), ["Paste", "-", "Select All"]);
    assert_eq!(picks.len(), 2, "one pick an item: the line is none");
}

#[test]
fn the_cell_under_the_pointer_is_inside_the_gutter() {
    let frame = Frame {
        columns: 80,
        lines: 24,
        ..Frame::default()
    };
    assert_eq!(
        cell_at(&frame, (8.0, 16.0), (16.0 + 8.0 * 3.5, 8.0 + 16.0 * 2.5)),
        Some((3, 2))
    );
    assert_eq!(
        cell_at(&frame, (8.0, 16.0), (4.0, 20.0)),
        None,
        "the gutter"
    );
    assert_eq!(
        cell_at(&frame, (8.0, 16.0), (16.0 + 8.0 * 80.0, 20.0)),
        None,
        "past the last"
    );
}

#[test]
fn the_slider_is_taken_where_pressed_and_follows_the_pointer_by_rows() {
    assert!((grip(110.0, (100.0, 40.0)) - 10.0).abs() < f64::EPSILON);
    assert!(
        (grip(10.0, (100.0, 40.0)) - 20.0).abs() < f64::EPSILON,
        "its middle, in the track"
    );
    let bar = Scrollbar {
        total: 1024,
        offset: 0,
        len: 24,
    };
    assert_eq!(bar_row(10.0, 10.0, (40.0, 440.0), bar), 0);
    assert_eq!(
        bar_row(210.0, 10.0, (40.0, 440.0), bar),
        500,
        "halfway: half the history"
    );
    assert_eq!(
        bar_row(900.0, 10.0, (40.0, 440.0), bar),
        1000,
        "held to the last"
    );
}

#[test]
fn the_geometry_is_in_pixels_with_the_gutter() {
    let geometry = geometry((800.0, 600.0), 2.0, (8.0, 16.0), false);
    assert_eq!((geometry.width, geometry.height), (1600, 1200));
    assert_eq!((geometry.cell_width, geometry.cell_height), (16, 32));
    assert_eq!((geometry.pad_left, geometry.pad_top), (32, 16));
    assert_eq!(
        super::geometry((10.0, 10.0), 1.0, (8.0, 16.0), true).pad_left,
        0
    );
}
