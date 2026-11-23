//! The frame a view draws, read from the core after known bytes: which rows changed, the ids that
//! stay with rows as they scroll, a full frame after a resize, cells, styles, wide characters,
//! the cursor and the selection.

use super::Render;
use crate::surface::{CellPixels, Color, Flags, Frame, Rgb, RowId, Shape, Wide};
use crate::terminal::Core;

fn core(rows: u16, columns: u16) -> Core {
    Core::new(rows, columns).unwrap()
}

fn drawn(render: &mut Render, core: &Core) -> Frame {
    let mut frame = Frame::default();
    assert!(render.frame(core, &mut frame, false), "a frame to draw");
    frame
}

/// Each row's text, cell by cell, a blank cell as a space.
fn texts(frame: &Frame) -> Vec<String> {
    let cell_text = |row: &crate::surface::Row| -> String {
        (row.cells.iter())
            .filter(|cell| cell.wide != Wide::Tail)
            .map(|cell| match row.text_of(cell) {
                "" => " ",
                text => text,
            })
            .collect::<String>()
            .trim_end()
            .to_string()
    };
    frame.rows.iter().map(cell_text).collect()
}

#[test]
fn the_first_frame_is_full_and_holds_every_row() {
    let mut core = core(3, 10);
    core.feed(b"hello\r\nworld");
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    assert!(frame.full);
    assert_eq!((frame.columns, frame.lines), (10, 3));
    assert_eq!(texts(&frame), ["hello", "world", ""]);
    assert_eq!(frame.cursor.at, Some((5, 1)));
    assert_eq!(frame.cursor.shape, Shape::Block);
}

#[test]
fn after_output_only_the_rows_it_changed_are_dirty_and_carry_cells() {
    let mut core = core(3, 10);
    core.feed(b"one\r\ntwo\r\nthree");
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    let mut frame = Frame::default();
    assert!(!render.frame(&core, &mut frame, false), "nothing changed");
    core.feed(b"\x1b[2;1Hdeux");
    let frame = drawn(&mut render, &core);
    assert!(!frame.full);
    // The row written and the row the cursor left.
    let dirty: Vec<u16> = (frame.rows.iter())
        .filter(|r| r.dirty)
        .map(|r| r.y)
        .collect();
    assert_eq!(dirty, [1, 2]);
    assert!(
        frame.rows[0].cells.is_empty(),
        "a clean row says only its id"
    );
    assert_eq!(texts(&frame)[1], "deux");
}

#[test]
fn a_row_keeps_its_id_as_output_scrolls_it_up() {
    let mut core = core(3, 10);
    core.feed(b"a\r\nb\r\nc");
    let mut render = Render::new().unwrap();
    let before = drawn(&mut render, &core);
    core.feed(b"\r\nd");
    let after = drawn(&mut render, &core);
    assert_eq!(texts(&after), ["b", "c", "d"]);
    assert_eq!(
        after.rows[0].id, before.rows[1].id,
        "b moved up with its id"
    );
    assert_eq!(after.rows[1].id, before.rows[2].id);
    let new = after.rows[2].id;
    assert!(
        before.rows.iter().all(|r| r.id != new),
        "the new row has a new id"
    );
}

#[test]
fn a_region_scrolled_in_place_gives_the_rows_new_ids() {
    let mut core = core(4, 10);
    core.feed(b"top\r\nx\r\ny\r\nbottom");
    let mut render = Render::new().unwrap();
    let before = drawn(&mut render, &core);
    // Rows 2 to 3 scroll by one within their region: the core renews the ids of what is in view,
    // so a view keeps nothing by an id that now shows other text.
    core.feed(b"\x1b[2;3r\x1b[3;1H\n\x1b[r");
    let after = drawn(&mut render, &core);
    assert_eq!(texts(&after), ["top", "y", "", "bottom"]);
    let old: Vec<RowId> = before.rows.iter().map(|r| r.id).collect();
    assert!(
        after
            .rows
            .iter()
            .all(|row| row.dirty && !old.contains(&row.id))
    );
}

#[test]
fn a_resize_makes_the_next_frame_full() {
    let mut core = core(3, 10);
    core.feed(b"hello");
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    core.resize(4, 12, CellPixels::default()).unwrap();
    let frame = drawn(&mut render, &core);
    assert!(frame.full);
    assert_eq!((frame.columns, frame.lines, frame.rows.len()), (12, 4, 4));
}

#[test]
fn a_wide_character_that_does_not_fit_goes_to_the_next_row() {
    let mut core = core(4, 5);
    core.feed("xy\r\nabcd日e".as_bytes());
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    assert_eq!(texts(&frame), ["xy", "abcd", "日e", ""]);
    let row = &frame.rows[1];
    assert!(row.wrapped);
    assert_eq!(
        row.cells[4].wide,
        Wide::Head,
        "the last column is left for the wrap"
    );
    let next = &frame.rows[2];
    assert_eq!(
        (next.cells[0].wide, next.cells[1].wide),
        (Wide::Wide, Wide::Tail)
    );
}

#[test]
fn styles_and_colours_are_the_programs() {
    let mut core = core(2, 20);
    core.feed(b"\x1b[1;31mred\x1b[0m \x1b[38;2;1;2;3;48;5;4mrgb\x1b[0m \x1b[4:3mcurl\x1b[0m");
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    let cells = &frame.rows[0].cells;
    assert_eq!(cells[0].style.fg, Color::Palette(1));
    assert!(cells[0].style.flags.has(Flags::BOLD));
    assert_eq!(cells[4].style.fg, Color::Rgb(Rgb { r: 1, g: 2, b: 3 }));
    assert_eq!(cells[4].style.bg, Some(frame.colors.palette[4]));
    assert_eq!(
        cells[3].style.bg, None,
        "a blank cell has the terminal's ground"
    );
    assert_eq!(cells[8].style.underline, crate::surface::Underline::Curly);
}

#[test]
fn the_theme_given_is_the_frames_colours() {
    let mut core = core(2, 10);
    let mut ansi = [Rgb::default(); 16];
    ansi[1] = Rgb::hex(0xff7b72);
    core.set_colors(&crate::surface::Palette {
        foreground: Rgb::hex(0xe6e6e6),
        background: Rgb::hex(0x1e1e1e),
        cursor: Rgb::hex(0xe6e6e6),
        ansi,
    });
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    assert_eq!(frame.colors.foreground, Rgb::hex(0xe6e6e6));
    assert_eq!(frame.colors.background, Rgb::hex(0x1e1e1e));
    assert_eq!(frame.colors.palette[1], Rgb::hex(0xff7b72));
}

#[test]
fn history_shows_in_the_scrollbar() {
    let mut core = core(3, 10);
    for n in 0..10 {
        core.feed(format!("{n}\r\n").as_bytes());
    }
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    assert_eq!(frame.scroll.len, 3);
    assert_eq!(frame.scroll.total, 11);
    assert_eq!(frame.scroll.offset, 8);
}

#[test]
fn a_program_on_the_other_screen_with_the_mouse_is_said_so() {
    let mut core = core(3, 10);
    core.feed(b"\x1b[?1049h\x1b[?1000h");
    let mut render = Render::new().unwrap();
    let frame = drawn(&mut render, &core);
    assert!(frame.alternate && frame.mouse_taken);
}
