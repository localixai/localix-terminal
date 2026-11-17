//! A laid-out row, the cursor and the scrollbar drawn into a canvas, the same on every platform:
//! where each ground, box piece, line and run goes in device pixels, worked out here, and only
//! the strokes themselves the platform's (`Pen`: Core Graphics on macOS, Cairo on Linux). The
//! terminal's ground is never painted, so what is behind the view shows through.

use super::boxes;
use super::layout::{Deco, Line, RowPaint, Run};
use super::overlay::CursorPaint;
use super::theme::Paint;
use crate::surface::{Shape, Underline};

/// A rectangle in device pixels, from its top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) const fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Where and how big the cells are, in pixels: a cell's size, the grid's top left, and the scale
/// of a point.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Grid {
    pub(crate) cell: (f64, f64),
    pub(crate) origin: (f64, f64),
    pub(crate) scale: f64,
}

/// A font's measures in pixels: a cell's width and height, from a cell's top to the baseline,
/// how far under the baseline an underline is and how thick, and how high a strike is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) cell: (f64, f64),
    pub(crate) baseline: f64,
    pub(crate) underline: (f64, f64),
    pub(crate) strike: f64,
}

/// A terminal's fonts as drawing needs them: their measures.
pub(crate) trait Faces {
    fn metrics(&self) -> Metrics;
}

/// What a platform's canvas draws with: a filled rectangle, an outline, a wave, dashes, and a
/// run of text whose characters keep to the grid's columns.
pub(crate) trait Pen {
    type Fonts: Faces;
    fn fill(&self, area: Rect, paint: Paint);
    /// The edge of `area`, `width` pixels thick, inside it.
    fn outline(&self, area: Rect, paint: Paint, width: f64);
    fn wave(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint);
    fn dashes(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint, dash: f64);
    /// `run` drawn with the grid's left and its baseline at `at`, each character at its column
    /// of cells `cell_width` pixels wide.
    fn text(&self, fonts: &Self::Fonts, run: &Run, at: (f64, f64), cell_width: f64);
    /// The canvas's width in pixels.
    fn width(&self) -> usize;
}

/// Draws `row` at the row `y` of `grid`: its grounds, its box pieces, its text and its lines.
pub(crate) fn row<P: Pen>(pen: &P, fonts: &P::Fonts, grid: &Grid, row: &RowPaint, y: u16) {
    let (width, height) = grid.cell;
    let top = grid.origin.1 + f64::from(y) * height;
    let left = grid.origin.0;
    for fill in &row.fills {
        let from = left + f64::from(fill.from) * width;
        let span = f64::from(fill.to.saturating_sub(fill.from)) * width;
        pen.fill(rect(from, top, span, height), fill.paint);
    }
    let line = grid.scale.round().max(1.0);
    for (column, character, paint) in &row.boxes {
        let x = left + f64::from(*column) * width;
        for piece in boxes::pieces(*character, width, height, line) {
            let shared = Paint::share(paint.rgb, f64::from(paint.alpha) / 255.0 * piece.share);
            pen.fill(
                rect(x + piece.x, top + piece.y, piece.width, piece.height),
                shared,
            );
        }
    }
    let baseline = top + fonts.metrics().baseline;
    for run in &row.runs {
        pen.text(fonts, run, (left, baseline), width);
    }
    for deco in &row.decos {
        decorate(pen, fonts, grid, deco, top);
    }
}

fn decorate<P: Pen>(pen: &P, fonts: &P::Fonts, grid: &Grid, deco: &Deco, top: f64) {
    let metrics = fonts.metrics();
    let (width, height) = grid.cell;
    let from = grid.origin.0 + f64::from(deco.from) * width;
    let span = f64::from(deco.to.saturating_sub(deco.from)) * width;
    let thick = metrics.underline.1.round().max(1.0);
    let under = (top + metrics.baseline + metrics.underline.0)
        .round()
        .min(top + height - thick);
    let bar = |y: f64| pen.fill(rect(from, y, span, thick), deco.paint);
    match deco.line {
        Line::Under(Underline::Double) => {
            bar(under - thick);
            bar(under + thick);
        }
        Line::Under(Underline::Curly) => pen.wave(from, span, under, thick, deco.paint),
        Line::Under(Underline::Dotted) => pen.dashes(from, span, under, thick, deco.paint, thick),
        Line::Under(Underline::Dashed) => {
            pen.dashes(from, span, under, thick, deco.paint, thick * 3.0);
        }
        Line::Under(Underline::Single | Underline::None) => bar(under),
        Line::Strike => bar((top + metrics.baseline - metrics.strike).round()),
        Line::Over => bar(top),
        Line::Edge => pen.outline(
            rect(from, top, span, height),
            deco.paint,
            grid.scale.round().max(1.0),
        ),
    }
}

/// Draws the cursor over the rows: a block with its character in the ground's colour, a bar or
/// an underline a point thick, or an outline.
pub(crate) fn cursor<P: Pen>(pen: &P, fonts: &P::Fonts, grid: &Grid, cursor: &CursorPaint) {
    let (width, height) = grid.cell;
    let left = grid.origin.0 + f64::from(cursor.column) * width;
    let top = grid.origin.1 + f64::from(cursor.y) * height;
    let span = f64::from(cursor.width) * width;
    let point = grid.scale.round().max(1.0);
    match cursor.shape {
        Shape::Block => pen.fill(rect(left, top, span, height), cursor.paint),
        Shape::Bar => pen.fill(rect(left, top, point, height), cursor.paint),
        Shape::Underline => pen.fill(rect(left, top + height - point, span, point), cursor.paint),
        Shape::Hollow => pen.outline(rect(left, top, span, height), cursor.paint, point),
    }
    if let Some(under) = &cursor.under {
        let baseline = top + fonts.metrics().baseline;
        pen.text(fonts, under, (grid.origin.0, baseline), width);
    }
}

/// Draws the scrollbar's slider at the right edge: `top` and `length` in pixels.
pub(crate) fn scrollbar<P: Pen>(pen: &P, top: f64, length: f64, width: f64, paint: Paint) {
    pen.fill(rect(pen.width() as f64 - width, top, width, length), paint);
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
