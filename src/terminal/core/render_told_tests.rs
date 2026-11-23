//! A frame for what a program changed with no row written, read from the core after known bytes:
//! the cursor moved, hidden, shown or reshaped, a ground colour set, a mode turned on. The
//! library's dirt covers rows only; the frame is new for these too, and the cursor's row carries
//! its cells for the character a block cursor stands on.

use super::Render;
use crate::surface::{Frame, Palette, Rgb};
use crate::terminal::Core;

fn core(rows: u16, columns: u16) -> Core {
    Core::new(rows, columns).unwrap()
}

fn drawn(render: &mut Render, core: &Core) -> Frame {
    let mut frame = Frame::default();
    assert!(render.frame(core, &mut frame, false), "a frame to draw");
    frame
}

#[test]
fn a_space_typed_over_a_blank_cell_moves_the_cursor_in_the_next_frame() {
    let mut core = core(3, 20);
    core.feed(b"$ ls");
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    // A shell echoes a space at the end of the line: the cell was blank and stays blank, only
    // the cursor moves.
    core.feed(b" ");
    let mut frame = Frame::default();
    assert!(
        render.frame(&core, &mut frame, false),
        "a space typed made no frame: the cursor stays where it was on screen"
    );
    assert_eq!(frame.cursor.at, Some((5, 0)));
    // The cursor moved by a sequence alone, nothing written.
    core.feed(b"\x1b[C");
    let mut frame = Frame::default();
    assert!(
        render.frame(&core, &mut frame, false),
        "a cursor move made no frame"
    );
    assert_eq!(frame.cursor.at, Some((6, 0)));
    // Back over the space, as Backspace echoes it.
    core.feed(b"\x08 \x08");
    let mut frame = Frame::default();
    assert!(
        render.frame(&core, &mut frame, false),
        "an erase made no frame"
    );
    assert_eq!(frame.cursor.at, Some((5, 0)));
}

#[test]
fn what_is_not_a_row_changing_alone_is_a_frame() {
    let mut core = core(3, 20);
    core.feed(b"$ ab");
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    let cases: [(&[u8], &str); 5] = [
        (b"\x08", "a backspace moving the cursor"),
        (b"\x1b[?25l", "the cursor hidden"),
        (b"\x1b[?25h", "the cursor shown"),
        (b"\x1b[5 q", "the cursor's shape"),
        (b"\x1b[?1000h", "the mouse taken"),
    ];
    for (bytes, what) in cases {
        core.feed(bytes);
        let mut frame = Frame::default();
        assert!(
            render.frame(&core, &mut frame, false),
            "{what} made no frame"
        );
    }
    let mut frame = Frame::default();
    assert!(!render.frame(&core, &mut frame, false), "nothing changed");
}

/// Whatever a program writes, the cursor the last frame drew is where the terminal's cursor is:
/// a seeded run of writes, cursor moves, erases, hides and shows, checked after each.
#[test]
fn the_cursor_drawn_is_always_the_terminals() {
    const STEPS: [&[u8]; 14] = [
        b"a",
        b" ",
        b"\x08",
        b"\x1b[C",
        b"\x1b[2D",
        b"\x1b[A",
        b"\x1b[B",
        b"\r",
        b"\n",
        b"\x1b[K",
        b"\x1b[3;7H",
        b"\x1b[?25l",
        b"\x1b[?25h",
        b"\x08 \x08",
    ];
    let mut core = core(6, 24);
    let mut render = Render::new().unwrap();
    let mut drawn_at = drawn(&mut render, &core).cursor.at;
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for step in 0..5000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let bytes = STEPS[usize::try_from(seed % 14).unwrap()];
        core.feed(bytes);
        let mut frame = Frame::default();
        if render.frame(&core, &mut frame, false) {
            drawn_at = frame.cursor.at;
        }
        let (row, column) = core.cursor();
        let truth = core.cursor_visible().then_some((column - 1, row - 1));
        assert_eq!(drawn_at, truth, "step {step}, after {bytes:?}");
    }
}

/// A block cursor draws the character it stands on: the cursor's row carries its cells in every
/// frame, though it stays clean for the pictures kept of it.
#[test]
fn the_cursors_row_carries_its_cells_when_only_the_cursor_moved() {
    let mut core = core(3, 20);
    core.feed(b"next\r\n$ ab");
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    // A shell's Left arrow: the cursor steps back onto the b.
    core.feed(b"\x08");
    let frame = drawn(&mut render, &core);
    assert_eq!(frame.cursor.at, Some((3, 1)));
    let row = frame.rows.iter().find(|row| row.y == 1).unwrap();
    assert!(!row.dirty, "the row did not change");
    let cell = row.cells.get(3).expect("the cursor's row has no cells");
    assert_eq!(row.text_of(cell), "b");
    let other = frame.rows.iter().find(|row| row.y == 0).unwrap();
    assert!(
        other.cells.is_empty(),
        "a clean row away from the cursor says only its id"
    );
}

/// A program's own ground colour (OSC 11) and its reset (OSC 111) are drawn as they are set,
/// though no row changed.
#[test]
fn a_ground_colour_a_program_sets_is_a_frame() {
    let mut core = core(3, 20);
    core.set_colors(&Palette {
        foreground: Rgb::hex(0x11_22_33),
        background: Rgb::hex(0xff_ff_ff),
        cursor: Rgb::hex(0),
        ansi: [Rgb::default(); 16],
    });
    let mut render = Render::new().unwrap();
    drawn(&mut render, &core);
    core.feed(b"\x1b]11;rgb:10/20/30\x1b\\");
    assert_eq!(
        drawn(&mut render, &core).colors.background,
        Rgb::hex(0x10_20_30)
    );
    core.feed(b"\x1b]111\x1b\\");
    assert_eq!(
        drawn(&mut render, &core).colors.background,
        Rgb::hex(0xff_ff_ff)
    );
}

/// A coding agent's prompt answers a space typed at its end by moving the cursor alone (the
/// trailing space draws nothing): these are the bytes one sends for it. The view draws the
/// cursor a cell further each time.
#[test]
fn a_space_an_agents_prompt_answers_with_a_cursor_move_moves_the_drawn_cursor() {
    let mut core = core(30, 100);
    core.feed(b"\x1b[26;3H> hi\x1b[26;5H");
    let mut render = Render::new().unwrap();
    assert_eq!(drawn(&mut render, &core).cursor.at, Some((4, 25)));
    core.feed(b"\x1b[?25l\x1b[26;6H\x1b[?25h");
    assert_eq!(drawn(&mut render, &core).cursor.at, Some((5, 25)));
    core.feed(b"\x1b[?25l\x1b[26;7H\x1b[?25h");
    assert_eq!(drawn(&mut render, &core).cursor.at, Some((6, 25)));
}
