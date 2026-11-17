//! What lies over a terminal's rows, laid out to draw: the cursor (a block with its character
//! in the ground's colour, a bar, an underline, or an outline where the pane is not in use), and
//! whether it blinks now; the line that says a program ended, under what it left; and the
//! scrollbar's slider.

use super::layout::{RowPaint, Run};
use super::theme::{Paint, Theme};
use crate::surface::{Flags, Frame, Shape, Wide};

/// A blinking cursor's period, in seconds, shown for its first half and hidden for the second
/// (a step, never a fade).
pub(crate) const BLINK: f64 = 1.0;

/// Whether the cursor blinks now: the program asked for it (DECSCUSR, mode 12), the view is in
/// sight with the keys in the key window (`in_use`), and the person did not ask for less motion.
pub(crate) const fn blinks(frame: &Frame, in_use: bool, less_motion: bool) -> bool {
    frame.cursor.blinking
        && frame.cursor.at.is_some()
        && frame.ended.is_none()
        && in_use
        && !less_motion
}

/// The cursor as drawn over the rows: its cell (two for a wide character), its look (an outline
/// where the pane is not in use), its colour, and the character under a block in the ground's
/// colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorPaint {
    pub(crate) column: u16,
    pub(crate) y: u16,
    pub(crate) width: u16,
    pub(crate) shape: Shape,
    pub(crate) paint: Paint,
    pub(crate) under: Option<Run>,
}

/// The cursor drawn over the rows; none while it is hidden, and once the program ended (the line
/// that says so takes its place).
pub(crate) fn cursor(frame: &Frame, theme: Theme) -> Option<CursorPaint> {
    if frame.ended.is_some() {
        return None;
    }
    let (mut column, y) = frame.cursor.at?;
    if frame.cursor.wide_tail {
        column = column.saturating_sub(1);
    }
    let row = frame.rows.iter().find(|row| row.y == y);
    let cell = row.and_then(|row| row.cells.get(usize::from(column)));
    let width = if cell.is_some_and(|cell| cell.wide == Wide::Wide) {
        2
    } else {
        1
    };
    let shape = if theme.active {
        frame.cursor.shape
    } else {
        Shape::Hollow
    };
    let paint = Paint::solid(
        frame
            .colors
            .cursor
            .unwrap_or_else(|| theme.palette().cursor),
    );
    let under = (row.zip(cell))
        .filter(|_| shape == Shape::Block)
        .and_then(|(row, cell)| {
            let text = row.text_of(cell);
            (!text.is_empty() && text != " ").then(|| Run {
                bold: cell.style.flags.has(Flags::BOLD),
                italic: cell.style.flags.has(Flags::ITALIC),
                paint: Paint::solid(theme.bg()),
                text: text.to_string(),
                columns: vec![(0, column)],
                end: column + width,
            })
        });
    Some(CursorPaint {
        column,
        y,
        width,
        shape,
        paint,
        under,
    })
}

/// The line that says the program ended ("(the program exited with 0)"), in the ink of dim text,
/// and the row it is on: the cursor's, where the cursor is at a line's start, else the one
/// under it; none while the program runs, or when that row is past the last.
pub(crate) fn ended(frame: &Frame, theme: Theme) -> Option<(u16, RowPaint)> {
    let words = frame.ended.as_deref()?;
    let (column, row) = frame
        .cursor
        .at
        .unwrap_or_else(|| (0, frame.lines.saturating_sub(1)));
    let y = if column == 0 {
        row
    } else {
        row.saturating_add(1)
    };
    if y >= frame.lines {
        return None;
    }
    let columns = (0u16..).zip(words.char_indices());
    let columns = columns.map(|(column, (at, _))| (u32::try_from(at).unwrap_or(u32::MAX), column));
    let columns: Vec<(u32, u16)> = columns.collect();
    let end = u16::try_from(columns.len()).unwrap_or(u16::MAX);
    let run = Run {
        bold: false,
        italic: false,
        paint: Paint::share(theme.fg(), 0.5),
        text: words.to_string(),
        columns,
        end,
    };
    let paint = RowPaint {
        fills: vec![],
        boxes: vec![],
        runs: vec![run],
        decos: vec![],
    };
    Some((y, paint))
}

/// The scrollbar's slider in a view `height` points tall, as its top and its length, when there
/// is history to scroll: its length the share of rows in view (at least 20), its place the
/// share scrolled.
pub(crate) fn scrollbar(frame: &Frame, height: f64) -> Option<(f64, f64)> {
    let bar = frame.scroll;
    if bar.total <= bar.len || bar.total == 0 {
        return None;
    }
    let share = |a: u64, b: u64| a as f64 / b as f64;
    let length = (share(bar.len, bar.total) * height)
        .floor()
        .max(20.0)
        .min(height)
        .round();
    let room = bar.total - bar.len;
    let top = (share(bar.offset.min(room), room) * (height - length)).round();
    Some((top, length))
}

/// The scrollbar's width, in points.
pub(crate) const SCROLLBAR_WIDTH: f64 = 14.0;

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
