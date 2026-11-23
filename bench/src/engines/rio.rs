//! rio's terminal core (`rio-vt`, its renderer, window and clipboard left out) with its own
//! parser.

use rio_vt::ansi::CursorShape;
use rio_vt::crosswords::pos::{Column, Line, Pos};
use rio_vt::crosswords::{Crosswords, CrosswordsSize};
use rio_vt::event::{VoidListener, WindowId};
use rio_vt::performer::handler::Processor;

use crate::{COLUMNS, Engine, HISTORY, ROWS, Screen};

pub struct Rio {
    term: Crosswords<VoidListener>,
    parser: Processor,
}

impl Rio {
    pub fn new() -> Self {
        let size = CrosswordsSize::new(usize::from(COLUMNS), usize::from(ROWS));
        let term = Crosswords::new(
            size,
            CursorShape::Block,
            VoidListener,
            WindowId::from(0),
            0,
            HISTORY,
        );
        Self {
            term,
            parser: Processor::default(),
        }
    }
}

impl Engine for Rio {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn screen(&self) -> Screen {
        // Row by row, by the core's own reader of a range, which puts each cell's marks with it.
        let last = Column(usize::from(COLUMNS) - 1);
        let rows = (0..i32::from(ROWS)).map(|row| {
            let (start, end) = (Pos::new(Line(row), Column(0)), Pos::new(Line(row), last));
            expanded(&self.term.bounds_to_string(start, end).replace('\n', ""))
        });
        let at = self.term.grid.cursor.pos;
        let cursor = (
            u16::try_from(at.row.0).unwrap_or(0),
            u16::try_from(at.col.0).unwrap_or(0),
        );
        Screen::new(cursor, rows)
    }
}

/// A row as the screen shows it: the reader gives a tab as one `\t` for the cells it spans, so
/// each is put back as the spaces to the next stop (every 8 columns, counted in characters).
fn expanded(row: &str) -> String {
    let mut out = String::with_capacity(row.len());
    let mut column = 0;
    for c in row.chars() {
        if c != '\t' {
            out.push(c);
            column += 1;
            continue;
        }
        let next = (column / 8 + 1) * 8;
        while column < next {
            out.push(' ');
            column += 1;
        }
    }
    out
}
