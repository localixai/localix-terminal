//! Selecting by the core's gestures: a drag selects the cells it crosses, a double click a word,
//! a triple click a line, Option and Command a block; a single click clears it; everything can
//! be selected; the selection's text joins wrapped lines.

use crate::surface::{Button, Geometry, Mods, Mouse, MouseAction};
use crate::terminal::Core;

const VIEW: Geometry = Geometry {
    width: 200,
    height: 80,
    cell_width: 10,
    cell_height: 20,
    pad_top: 0,
    pad_bottom: 0,
    pad_left: 0,
    pad_right: 0,
};

fn at(column: f32, row: f32, action: MouseAction, time_ms: u64) -> Mouse {
    Mouse {
        x: column * 10.0 + 2.0,
        y: row * 20.0 + 5.0,
        button: Some(Button::Left),
        action,
        time_ns: time_ms * 1_000_000,
        ..Mouse::default()
    }
}

fn core() -> Core {
    let mut core = Core::new(4, 20).unwrap();
    core.feed(b"hello world\r\nsecond line here");
    core
}

#[test]
fn a_drag_selects_what_it_crosses() {
    let mut core = core();
    core.select(&at(0.0, 0.0, MouseAction::Press, 0), &VIEW);
    // A drag takes in a cell once the pointer is past its middle.
    core.select(&at(4.6, 0.0, MouseAction::Motion, 10), &VIEW);
    core.select(&at(4.6, 0.0, MouseAction::Release, 20), &VIEW);
    assert_eq!(core.selection_text().as_deref(), Some("hello"));
    core.select(&at(9.0, 3.0, MouseAction::Press, 2000), &VIEW);
    assert_eq!(core.selection_text(), None, "a click clears it");
}

#[test]
fn a_double_click_selects_a_word_and_a_triple_click_the_line() {
    let mut core = core();
    for time in [0, 100] {
        core.select(&at(8.0, 0.0, MouseAction::Press, time), &VIEW);
        core.select(&at(8.0, 0.0, MouseAction::Release, time + 10), &VIEW);
    }
    assert_eq!(core.selection_text().as_deref(), Some("world"));
    core.select(&at(8.0, 0.0, MouseAction::Press, 200), &VIEW);
    assert_eq!(core.selection_text().as_deref(), Some("hello world"));
}

#[test]
fn option_and_command_drag_a_block() {
    let mut core = core();
    let block = |mouse: Mouse| Mouse {
        mods: Mods(Mods::ALT | Mods::SUPER),
        ..mouse
    };
    core.select(&block(at(1.0, 0.0, MouseAction::Press, 0)), &VIEW);
    core.select(&block(at(3.6, 1.0, MouseAction::Motion, 10)), &VIEW);
    assert_eq!(core.selection_text().as_deref(), Some("ell\neco"));
}

#[test]
fn all_is_selected_and_wrapped_lines_copy_whole() {
    let mut core = Core::new(4, 10).unwrap();
    core.feed(b"a long line that wraps\r\nnext");
    core.select_all();
    assert_eq!(
        core.selection_text().as_deref(),
        Some("a long line that wraps\nnext")
    );
}

#[test]
fn a_drag_over_no_cell_leaves_the_selection_as_it_is() {
    let mut core = core();
    core.select(&at(0.0, 0.0, MouseAction::Press, 0), &VIEW);
    core.select(&at(4.6, 0.0, MouseAction::Motion, 10), &VIEW);
    assert_eq!(core.selection_text().as_deref(), Some("hello"));
    core.select_all();
    let all = core.selection_text();
    assert_ne!(all.as_deref(), Some("hello"));
    // A drag the core finds no cell under never reuses the cell the last drag was over.
    core.gestured(&at(8.0, 1.0, MouseAction::Motion, 20), &VIEW, None);
    assert_eq!(core.selection_text(), all);
}
