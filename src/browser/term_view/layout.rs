//! A terminal's frame laid out to draw, the same on every platform: for each row, the grounds of
//! its cells, the selection and the matches over them, its runs of text (cells of one face and
//! one colour, each character pinned to its column, a wide character's second half no break, so
//! an emoji sequence the core keeps over several cells is shaped as one) and its lines
//! (underlines, strikes, the link pointed at); around them, the gutter. All in columns and rows
//! but the gutter, which a canvas turns into pixels by its cell size. What lies over the rows is
//! `overlay.rs`.

use std::hash::{Hash, Hasher};

use super::theme::{Paint, Theme};
use crate::surface::{Cell, Color, Colors, Flags, Frame, Rgb, Row, Style, Underline, Wide};

/// The gutter around the grid, in points: a margin on every side, and none but under the last row
/// for a program on the other screen, which draws to the edge itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gutter {
    pub(crate) top: u32,
    pub(crate) right: u32,
    pub(crate) bottom: u32,
    pub(crate) left: u32,
}

pub(crate) const fn gutter(alternate: bool) -> Gutter {
    match alternate {
        false => Gutter {
            top: 8,
            right: 16,
            bottom: 8,
            left: 16,
        },
        true => Gutter {
            top: 0,
            right: 0,
            bottom: 8,
            left: 0,
        },
    }
}

/// The grid a view of `width` by `height` points holds in cells of `cell` points: at least two
/// columns and one row.
pub(crate) fn grid(width: f64, height: f64, cell: (f64, f64), alternate: bool) -> (u16, u16) {
    let pad = gutter(alternate);
    let fit = |room: f64, size: f64, least: u16| {
        let n = (room / size).floor();
        match n.is_finite() && n >= f64::from(least) {
            true => whole(n),
            false => least,
        }
    };
    let width = width - f64::from(pad.left + pad.right);
    let height = height - f64::from(pad.top + pad.bottom);
    (fit(width, cell.0, 2), fit(height, cell.1, 1))
}

/// `n` (whole, at least 0) as a count, held to what a count of cells can be.
fn whole(n: f64) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the range of a u16 just before"
    )]
    let n = n.clamp(0.0, f64::from(u16::MAX)) as u16;
    n
}

/// A stretch of columns, `from` to before `to`, covered by `paint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Fill {
    pub(crate) from: u16,
    pub(crate) to: u16,
    pub(crate) paint: Paint,
}

/// Characters of one face and one colour: `text`, the column each cell's text starts at (by its
/// byte offset in `text`), and the column after its last cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Run {
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) paint: Paint,
    pub(crate) text: String,
    pub(crate) columns: Vec<(u32, u16)>,
    pub(crate) end: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Line {
    Under(Underline),
    Strike,
    Over,
    /// The edge of the match a find shows.
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Deco {
    pub(crate) from: u16,
    pub(crate) to: u16,
    pub(crate) line: Line,
    pub(crate) paint: Paint,
}

/// What a row draws, in order: its grounds, then its box characters (drawn as lines, by
/// `boxes.rs`) and its text, then its lines.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct RowPaint {
    pub(crate) fills: Vec<Fill>,
    pub(crate) boxes: Vec<(u16, char, Paint)>,
    pub(crate) runs: Vec<Run>,
    pub(crate) decos: Vec<Deco>,
}

/// What a row draws as `theme` shows `frame`, and a key that is the same for any row that draws
/// the same (its cells, its selection, its matches), so a row drawn once is not drawn again.
pub(crate) fn row(row: &Row, frame: &Frame, theme: Theme) -> (RowPaint, u64) {
    let mut paint = RowPaint::default();
    grounds(row, &frame.colors, &mut paint);
    if let Some((from, to)) = row.selected {
        paint.fills.push(Fill {
            from,
            to: to.saturating_add(1),
            paint: theme.selection(),
        });
    }
    for span in frame.matches.iter().filter(|span| span.y == row.y) {
        let shown = frame.current == Some(*span);
        let (from, to) = (span.from, span.to.saturating_add(1));
        paint.fills.push(Fill {
            from,
            to,
            paint: theme.found(shown),
        });
        if shown {
            paint.decos.push(Deco {
                from,
                to,
                line: Line::Edge,
                paint: Paint::solid(theme.accent()),
            });
        }
    }
    let linked: Vec<(u16, u16)> = (frame.link.iter())
        .filter(|span| span.y == row.y)
        .map(|span| (span.from, span.to))
        .collect();
    texts(row, (&frame.colors, &linked), theme, &mut paint);
    let mut hasher = std::hash::DefaultHasher::new();
    paint.hash(&mut hasher);
    (paint, hasher.finish())
}

/// A cell's text and ground colours, its own or the terminal's, swapped when it is inverse; no
/// ground where it is the terminal's, which is clear.
fn colours(style: &Style, colors: &Colors) -> (Rgb, Option<Rgb>) {
    let bold = style.flags.has(Flags::BOLD);
    let fg = match style.fg {
        Color::Default => colors.foreground,
        // Bold text in one of the first eight colours is drawn in its bright one.
        Color::Palette(at) if bold && at < 8 => colors.palette[usize::from(at) + 8],
        Color::Palette(at) => colors.palette[usize::from(at)],
        Color::Rgb(rgb) => rgb,
    };
    match style.flags.has(Flags::INVERSE) {
        true => (style.bg.unwrap_or(colors.background), Some(fg)),
        false => (fg, style.bg),
    }
}

fn grounds(row: &Row, colors: &Colors, paint: &mut RowPaint) {
    for (column, cell) in (0u16..).zip(&row.cells) {
        let Some(bg) = colours(&cell.style, colors).1 else {
            continue;
        };
        let fill = Paint::solid(bg);
        match paint.fills.last_mut() {
            Some(last) if last.to == column && last.paint == fill => last.to = column + 1,
            _ => paint.fills.push(Fill {
                from: column,
                to: column + 1,
                paint: fill,
            }),
        }
    }
}

/// Cells a run of text, and the lines under, through and over them; `linked`, the stretches of
/// the link pointed at in this row.
fn texts(
    row: &Row,
    (colors, linked): (&Colors, &[(u16, u16)]),
    theme: Theme,
    paint: &mut RowPaint,
) {
    let mut next = None;
    for (column, cell) in (0u16..).zip(&row.cells) {
        let text = row.text_of(cell);
        let style = &cell.style;
        let link = (linked.iter()).any(|(from, to)| (*from..=*to).contains(&column));
        let lines = (column, link);
        // The second half of a wide character is drawn by the first, and breaks no run.
        if cell.wide == Wide::Tail {
            decorations(lines, cell, colors, theme, paint);
            continue;
        }
        if text.is_empty() || text == " " || style.flags.has(Flags::INVISIBLE) {
            next = None;
            decorations(lines, cell, colors, theme, paint);
            continue;
        }
        let ink = ink(style, colors, theme);
        if let Some(line) = boxed(text) {
            paint.boxes.push((column, line, ink));
            next = None;
            decorations(lines, cell, colors, theme, paint);
            continue;
        }
        let (bold, italic) = (style.flags.has(Flags::BOLD), style.flags.has(Flags::ITALIC));
        let joins = next == Some(column)
            && (paint.runs.last())
                .is_some_and(|run: &Run| (run.bold, run.italic, run.paint) == (bold, italic, ink));
        if !joins {
            paint.runs.push(Run {
                bold,
                italic,
                paint: ink,
                text: String::new(),
                columns: vec![],
                end: column,
            });
        }
        let after = column + if cell.wide == Wide::Wide { 2 } else { 1 };
        if let Some(run) = paint.runs.last_mut() {
            run.columns
                .push((u32::try_from(run.text.len()).unwrap_or(u32::MAX), column));
            run.text.push_str(text);
            run.end = after;
        }
        next = Some(after);
        decorations(lines, cell, colors, theme, paint);
    }
}

/// The one character of `text` when it is one drawn as lines.
fn boxed(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    (chars.next().is_none() && super::boxes::drawn(first)).then_some(first)
}

/// The colour text is drawn in: its own, made to read on its ground, at half for faint text.
fn ink(style: &Style, colors: &Colors, theme: Theme) -> Paint {
    let (fg, bg) = colours(style, colors);
    let fg = theme.readable(fg, bg.unwrap_or_else(|| theme.bg()));
    match style.flags.has(Flags::FAINT) {
        true => Paint::share(fg, 0.5),
        false => Paint::solid(fg),
    }
}

/// The lines of `cell` at `column`, underlined too where it is part of the link pointed at.
fn decorations(
    (column, link): (u16, bool),
    cell: &Cell,
    colors: &Colors,
    theme: Theme,
    paint: &mut RowPaint,
) {
    let style = &cell.style;
    let width = if cell.wide == Wide::Wide { 2 } else { 1 };
    let ink = ink(style, colors, theme);
    let mut add = |line: Line, paint_with: Paint| match paint.decos.last_mut() {
        Some(last) if last.to == column && last.line == line && last.paint == paint_with => {
            last.to = column + width
        }
        _ => paint.decos.push(Deco {
            from: column,
            to: column + width,
            line,
            paint: paint_with,
        }),
    };
    if cell.wide == Wide::Tail {
        return;
    }
    if style.underline != Underline::None {
        let under = match style.underline_color {
            Color::Default => ink,
            Color::Palette(at) => Paint::solid(colors.palette[usize::from(at)]),
            Color::Rgb(rgb) => Paint::solid(rgb),
        };
        add(Line::Under(style.underline), under);
    } else if link {
        add(Line::Under(Underline::Single), Paint::solid(ink.rgb));
    }
    if style.flags.has(Flags::STRIKE) {
        add(Line::Strike, ink);
    }
    if style.flags.has(Flags::OVERLINE) {
        add(Line::Over, ink);
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
