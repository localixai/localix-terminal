//! Every cell a frame reads, set against every field the library says of it: a seeded run of
//! text, styles, grounds, wide and joined characters and erases, and at each cell of each row
//! the frame's own reading (which asks only what a cell needs) and one that asks the library
//! for everything must agree.

use ghostty_vt_sys as vt;

use super::super::cell;
use super::super::cell_bits::{self, asked};
use super::super::done;
use super::super::style::{color, rgb, underline, width};
use super::Render;
use crate::surface::{Cell, Flags, Style};
use crate::terminal::Core;

/// The cell `cells` is at, every field asked of the library.
fn every_field(cells: vt::RenderStateRowCells, text: &mut String) -> Cell {
    fn get<T: Default>(
        cells: vt::RenderStateRowCells,
        data: vt::RenderStateRowCellsData::Type,
    ) -> Option<T> {
        let mut value = T::default();
        let out: *mut T = &raw mut value;
        // SAFETY: `out` points to the type `data` is written as; the iterator is at a cell.
        let read = unsafe { vt::ghostty_render_state_row_cells_get(cells, data, out.cast()) };
        done(read, "a cell").ok().map(|()| value)
    }
    let from = u32::try_from(text.len()).unwrap();
    let mut bytes = [0u8; 64];
    let mut buffer = vt::Buffer {
        ptr: bytes.as_mut_ptr(),
        cap: bytes.len(),
        len: 0,
    };
    let out: *mut vt::Buffer = &raw mut buffer;
    let data = vt::RenderStateRowCellsData::GRAPHEMES_UTF8;
    // SAFETY: the buffer points to `bytes`, its room said; the iterator is at a cell.
    let read = unsafe { vt::ghostty_render_state_row_cells_get(cells, data, out.cast()) };
    if read == vt::Result::SUCCESS {
        text.push_str(&String::from_utf8_lossy(
            &bytes[..buffer.len.min(bytes.len())],
        ));
    }
    let raw: vt::Cell = get(cells, vt::RenderStateRowCellsData::RAW).unwrap_or_default();
    let style: vt::Style = get(cells, vt::RenderStateRowCellsData::STYLE).unwrap_or_default();
    let bg: Option<vt::ColorRgb> = get(cells, vt::RenderStateRowCellsData::BG_COLOR);
    Cell {
        from,
        len: u16::try_from(text.len() - from as usize).unwrap(),
        wide: width(asked(raw).wide),
        style: Style {
            fg: color(style.fg_color),
            bg: bg.map(rgb),
            flags: Flags(style.flags.to_le_bytes()[0]),
            underline: underline(style.flags),
            underline_color: color(style.underline_color),
        },
    }
}

/// How many cells of `core`'s screen were read both ways; panics where the two differ.
fn compare_every_cell(render: &mut Render, core: &Core) -> usize {
    render.begin(core).unwrap();
    let out: *mut vt::RenderStateRowIterator = &raw mut render.rows;
    // SAFETY: the iterator made in `new` is pointed at this render state's rows.
    let read = unsafe {
        vt::ghostty_render_state_get(render.state, vt::RenderStateData::ROW_ITERATOR, out.cast())
    };
    assert_eq!(read, vt::Result::SUCCESS);
    let mut compared = 0;
    // SAFETY: the iterator was set just now, over a render state not updated since.
    while unsafe { vt::ghostty_render_state_row_iterator_next(render.rows) } {
        let out: *mut vt::RenderStateRowCells = &raw mut render.cells;
        // SAFETY: the cell iterator made in `new` is pointed at the current row's cells.
        let read = unsafe {
            vt::ghostty_render_state_row_get(render.rows, vt::RenderStateRowData::CELLS, out.cast())
        };
        assert_eq!(read, vt::Result::SUCCESS);
        let mut view = vt::CellsView::default();
        let out: *mut vt::CellsView = &raw mut view;
        // SAFETY: a row's raw cells are written as a cells view.
        let read = unsafe {
            vt::ghostty_render_state_row_get(
                render.rows,
                vt::RenderStateRowData::CELLS_RAW,
                out.cast(),
            )
        };
        assert_eq!(read, vt::Result::SUCCESS);
        // SAFETY: `len` cells at `ptr`, valid until the render state is next updated.
        let values = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let (mut quick, mut whole) = (String::new(), String::new());
        for (x, &value) in (0..=u16::MAX).zip(values) {
            let fields = cell_bits::fields(cell_bits::places(), value);
            let a = cell::read(render.cells, fields, x, &mut quick);
            // SAFETY: the cell iterator is over this row, and `x` one of its columns.
            let at = unsafe { vt::ghostty_render_state_row_cells_select(render.cells, x) };
            assert_eq!(at, vt::Result::SUCCESS);
            let b = every_field(render.cells, &mut whole);
            assert_eq!((a, &quick), (b, &whole), "cell {compared}");
            compared += 1;
        }
    }
    let mut frame = crate::surface::Frame::default();
    render.finish(&mut frame, true);
    compared
}

const STEPS: &[&[u8]] = &[
    b"plain text ",
    b"\x1b[1;31mbold red\x1b[0m ",
    b"\x1b[38;5;208;48;5;19mpalette\x1b[0m",
    b"\x1b[38;2;10;20;30;48;2;200;100;50mrgb\x1b[m",
    b"\x1b[4:3;58;2;1;2;3mcurly\x1b[24;59m",
    b"\x1b[7;9;2;3mmany\x1b[m",
    b"\x1b[44m\x1b[K\x1b[m",
    b"\x1b[48;5;3m\x1b[5X\x1b[m",
    b"\xe4\xb8\xad\xe6\x96\x87",
    b"\xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x92\xbb",
    b"e\xcc\x81a\xcc\x8a",
    b"\xe2\x9d\xa4\xef\xb8\x8f",
    b"\r\n",
    b"\x1b[H",
    b"\x1b[3;40H",
    b"\x1b[2K",
    b"\x1b[41m \x1b[49m",
    b"\xff\xfe",
    b"\x1b[8mhidden\x1b[28m",
];

#[test]
fn a_frame_reads_every_cell_as_every_field_says_it() {
    let mut core = Core::new(24, 80).unwrap();
    let mut render = Render::new().unwrap();
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut compared = 0;
    for step in 0..3_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        core.feed(STEPS[usize::try_from(seed % STEPS.len() as u64).unwrap()]);
        if step % 50 == 0 {
            compared += compare_every_cell(&mut render, &core);
        }
    }
    assert!(compared > 100_000, "read {compared} cells");
}
