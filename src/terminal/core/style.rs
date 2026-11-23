//! A cell's look as the core keeps it, read as the view's: colours (the palette's, a program's
//! own, or the theme's), the underline's kind, a wide character's halves, and a row's own flags.

use ghostty_vt_sys as vt;

use crate::surface::{Color, Rgb, Underline, Wide};

pub(super) fn rgb(color: vt::ColorRgb) -> Rgb {
    Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

pub(super) fn color(color: vt::StyleColor) -> Color {
    match vt::StyleColorTag::Type::from(color.tag) {
        // SAFETY: the tag says the palette index is the field set.
        vt::StyleColorTag::PALETTE => Color::Palette(unsafe { color.value.palette }),
        // SAFETY: the tag says the colour is the field set.
        vt::StyleColorTag::RGB => Color::Rgb(rgb(unsafe { color.value.rgb })),
        _ => Color::Default,
    }
}

pub(super) fn underline(flags: vt::StyleFlags) -> Underline {
    match (flags >> vt::STYLE_UNDERLINE_SHIFT) & 0b111 {
        1 => Underline::Single,
        2 => Underline::Double,
        3 => Underline::Curly,
        4 => Underline::Dotted,
        5 => Underline::Dashed,
        _ => Underline::None,
    }
}

/// A cell's width as the library names it.
pub(super) fn width(wide: vt::CellWide::Type) -> Wide {
    match wide {
        vt::CellWide::WIDE => Wide::Wide,
        vt::CellWide::SPACER_TAIL => Wide::Tail,
        vt::CellWide::SPACER_HEAD => Wide::Head,
        _ => Wide::Narrow,
    }
}

/// Whether a raw row says `data` (a bool) of itself.
pub(super) fn raw_row(row: vt::Row, data: vt::RowData::Type) -> bool {
    let mut value = false;
    let out: *mut bool = &raw mut value;
    // SAFETY: the row's flag is written as a bool.
    let _ = unsafe { vt::ghostty_row_get(row, data, out.cast()) };
    value
}
