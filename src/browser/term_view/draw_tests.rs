//! Where the strokes of a row go, whatever the platform draws them with: a ground over its cells,
//! text at the grid's left and the font's baseline, an underline under the baseline, a cursor of
//! each shape, and the scrollbar at the right edge.

use std::sync::Mutex;

use super::{Faces, Grid, Metrics, Pen, Rect, cursor, rect, row, scrollbar};
use crate::browser::term_view::layout::{Deco, Fill, Line, RowPaint, Run};
use crate::browser::term_view::overlay::CursorPaint;
use crate::browser::term_view::theme::Paint;
use crate::surface::{Rgb, Shape, Underline};

struct Font;

impl Faces for Font {
    fn metrics(&self) -> Metrics {
        Metrics {
            cell: (8.0, 16.0),
            baseline: 12.0,
            underline: (2.0, 1.0),
            strike: 4.0,
        }
    }
}

/// A pen that writes down what it was asked to draw.
#[derive(Default)]
struct Notes(Mutex<Vec<String>>);

impl Notes {
    fn say(&self, note: String) {
        self.0.lock().unwrap().push(note);
    }

    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Pen for Notes {
    type Fonts = Font;

    fn fill(&self, area: Rect, _paint: Paint) {
        self.say(format!(
            "fill {} {} {} {}",
            area.x, area.y, area.width, area.height
        ));
    }

    fn outline(&self, area: Rect, _paint: Paint, width: f64) {
        self.say(format!(
            "edge {} {} {} {} {width}",
            area.x, area.y, area.width, area.height
        ));
    }

    fn wave(&self, from: f64, span: f64, y: f64, _thick: f64, _paint: Paint) {
        self.say(format!("wave {from} {span} {y}"));
    }

    fn dashes(&self, from: f64, span: f64, y: f64, _thick: f64, _paint: Paint, dash: f64) {
        self.say(format!("dash {from} {span} {y} {dash}"));
    }

    fn text(&self, _fonts: &Font, run: &Run, (left, baseline): (f64, f64), cell_width: f64) {
        self.say(format!("text {} {left} {baseline} {cell_width}", run.text));
    }

    fn width(&self) -> usize {
        400
    }
}

const GRID: Grid = Grid {
    cell: (8.0, 16.0),
    origin: (16.0, 8.0),
    scale: 1.0,
};

fn ink() -> Paint {
    Paint::solid(Rgb::hex(0x00ff00))
}

#[test]
fn a_rows_ground_text_and_underline_go_by_its_cells_and_the_fonts_baseline() {
    let paint = RowPaint {
        fills: vec![Fill {
            from: 2,
            to: 5,
            paint: ink(),
        }],
        boxes: vec![],
        runs: vec![Run {
            bold: false,
            italic: false,
            paint: ink(),
            text: "hey".into(),
            columns: vec![(0, 2), (1, 3), (2, 4)],
            end: 5,
        }],
        decos: vec![Deco {
            from: 2,
            to: 5,
            line: Line::Under(Underline::Single),
            paint: ink(),
        }],
    };
    let notes = Notes::default();
    row(&notes, &Font, &GRID, &paint, 1);
    assert_eq!(
        notes.all(),
        [
            "fill 32 24 24 16",
            "text hey 16 36 8",
            // The baseline's 36, two under it: within the row.
            "fill 32 38 24 1",
        ]
    );
}

#[test]
fn a_cursor_is_a_block_a_bar_an_underline_or_an_edge() {
    let notes = Notes::default();
    for shape in [Shape::Block, Shape::Bar, Shape::Underline, Shape::Hollow] {
        let paint = CursorPaint {
            column: 1,
            y: 0,
            width: 1,
            shape,
            paint: ink(),
            under: None,
        };
        cursor(&notes, &Font, &GRID, &paint);
    }
    assert_eq!(
        notes.all(),
        [
            "fill 24 8 8 16",
            "fill 24 8 1 16",
            "fill 24 23 8 1",
            "edge 24 8 8 16 1"
        ]
    );
}

#[test]
fn the_slider_is_at_the_right_edge() {
    let notes = Notes::default();
    scrollbar(&notes, 10.0, 50.0, 14.0, ink());
    assert_eq!(notes.all(), ["fill 386 10 14 50"]);
    assert_eq!(rect(1.0, 2.0, 3.0, 4.0).height, 4.0);
}
