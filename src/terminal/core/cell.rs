//! One cell of a row as a frame carries it, read at the render state's cell iterator: its text,
//! width, style and ground. Most cells on a screen are one codepoint with no style of their
//! own, and those are read from the raw cell's bits alone (`cell_bits`); only a cell of several codepoints asks the
//! library for its text, and only a styled cell, or one that is ground alone, for its style and
//! ground. A frame reads every cell of every changed row, so this is the hot path of a frame.

use ghostty_vt_sys as vt;

use super::cell_bits::Fields;
use super::done;
use super::style::{color, rgb, underline, width};
use crate::surface::{Cell, Flags, Style};

/// Column `x` of the row `cells` is over, `fields` what its raw value says, its text added to
/// `text`. The iterator is moved to the cell only when the library must be asked of it.
pub(super) fn read(
    cells: vt::RenderStateRowCells,
    fields: Fields,
    x: u16,
    text: &mut String,
) -> Cell {
    let from = u32::try_from(text.len()).unwrap_or(u32::MAX);
    let Fields {
        tag,
        codepoint,
        styled,
        wide,
    } = fields;
    if tag == vt::CellContentTag::CODEPOINT && !styled {
        if codepoint != 0 {
            text.push(char::from_u32(codepoint).unwrap_or(char::REPLACEMENT_CHARACTER));
        }
        let len = text.len().saturating_sub(from as usize);
        return Cell {
            from,
            len: u16::try_from(len).unwrap_or(u16::MAX),
            wide: width(wide),
            style: Style::default(),
        };
    }
    // SAFETY: the iterator is over this row's cells and `x` is one of its columns.
    if unsafe { vt::ghostty_render_state_row_cells_select(cells, x) } != vt::Result::SUCCESS {
        return Cell::default();
    }
    match tag {
        vt::CellContentTag::CODEPOINT => {
            if codepoint != 0 {
                text.push(char::from_u32(codepoint).unwrap_or(char::REPLACEMENT_CHARACTER));
            }
        }
        vt::CellContentTag::CODEPOINT_GRAPHEME => cluster(cells, text),
        _ => {}
    }
    let style: vt::Style = if styled {
        get(cells, vt::RenderStateRowCellsData::STYLE).unwrap_or_default()
    } else {
        vt::Style::default()
    };
    // A ground is the cell's own when the cell is ground alone or its style chooses one; the
    // library resolves either to a colour.
    let ground_alone = !matches!(
        tag,
        vt::CellContentTag::CODEPOINT | vt::CellContentTag::CODEPOINT_GRAPHEME
    );
    let grounded = ground_alone || i32::from(style.bg_color.tag) != vt::StyleColorTag::NONE;
    let bg: Option<vt::ColorRgb> = grounded
        .then(|| get(cells, vt::RenderStateRowCellsData::BG_COLOR))
        .flatten();
    let len = text.len().saturating_sub(from as usize);
    Cell {
        from,
        len: u16::try_from(len).unwrap_or(u16::MAX),
        wide: width(wide),
        style: Style {
            fg: color(style.fg_color),
            bg: bg.map(rgb),
            flags: Flags(style.flags.to_le_bytes()[0]),
            underline: underline(style.flags),
            underline_color: color(style.underline_color),
        },
    }
}

/// The text of a cell of several codepoints, as the library writes it out.
fn cluster(cells: vt::RenderStateRowCells, text: &mut String) {
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
    let written = bytes.get(..buffer.len.min(bytes.len())).unwrap_or_default();
    if read == vt::Result::SUCCESS {
        text.push_str(&String::from_utf8_lossy(written));
    }
}

/// What the iterator's cell says of `data`, when it says it.
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
