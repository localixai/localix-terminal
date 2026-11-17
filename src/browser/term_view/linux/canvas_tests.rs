//! The Cairo canvas: its ground is clear, a fill lands on the pixels the shared drawing named, in
//! premultiplied red, green, blue and alpha, and a run of text puts ink only inside its cells.

use super::super::super::draw::{Faces, Grid, Pen, rect, row};
use super::super::super::layout::{RowPaint, Run};
use super::super::super::painter::{Backend, pixels};
use super::super::super::theme::Paint;
use super::Canvas;
use crate::surface::Rgb;

fn pixel(rgba: &[u8], width: usize, (x, y): (usize, usize)) -> [u8; 4] {
    let at = (y * width + x) * 4;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

#[test]
fn the_ground_is_clear_and_a_fill_is_premultiplied_where_it_was_asked() {
    let canvas = Canvas::canvas(20, 10).unwrap();
    canvas.fill(
        rect(10.0, 0.0, 10.0, 10.0),
        Paint::share(Rgb::hex(0xff0000), 0.5),
    );
    let rgba = canvas.rgba();
    assert_eq!(rgba.len(), 20 * 10 * 4);
    assert_eq!(
        pixel(&rgba, 20, (2, 5)),
        [0, 0, 0, 0],
        "nothing drawn: clear"
    );
    let [r, g, b, a] = pixel(&rgba, 20, (15, 5));
    assert!((126..=129).contains(&a), "half: {a}");
    assert_eq!((r, g, b), (a, 0, 0), "premultiplied");
}

#[test]
fn a_run_of_text_inks_its_own_cells_only() {
    let Some(fonts) = Canvas::fonts("monospace", 16.0, true) else {
        return;
    };
    let cell = fonts.metrics().cell;
    let grid = Grid {
        cell: (cell.0.round().max(1.0), cell.1.round().max(1.0)),
        origin: (0.0, 0.0),
        scale: 1.0,
    };
    let (column, tall) = (pixels(grid.cell.0), pixels(grid.cell.1));
    let width = 8 * column;
    let canvas = Canvas::canvas(width, tall).unwrap();
    let paint = RowPaint {
        fills: vec![],
        boxes: vec![],
        runs: vec![Run {
            bold: false,
            italic: false,
            paint: Paint::solid(Rgb::hex(0xffffff)),
            text: "MM".into(),
            columns: vec![(0, 2), (1, 3)],
            end: 4,
        }],
        decos: vec![],
    };
    row(&canvas, &fonts, &grid, &paint, 0);
    let rgba = canvas.rgba();
    let inked = |from: usize, to: usize| {
        (0..tall).any(|y| (from..to).any(|x| pixel(&rgba, width, (x, y))[3] > 0))
    };
    assert!(inked(2 * column, 4 * column), "the run's cells have ink");
    assert!(!inked(0, column), "the cells before it none");
    assert!(!inked(5 * column, 8 * column), "nor those after it");
}
