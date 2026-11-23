//! `alacritty_terminal`'s core with its own parser (`vte`'s ANSI processor, synchronized output
//! handled as it handles it): the terminal core of a GPU terminal, and of others that embed it.

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::Processor;

use crate::{COLUMNS, Engine, HISTORY, ROWS, Screen};

/// The bench's size as the core asks for one.
struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        usize::from(ROWS)
    }

    fn screen_lines(&self) -> usize {
        usize::from(ROWS)
    }

    fn columns(&self) -> usize {
        usize::from(COLUMNS)
    }
}

pub struct Alacritty {
    term: Term<VoidListener>,
    parser: Processor,
}

impl Alacritty {
    pub fn new() -> Self {
        let config = Config {
            scrolling_history: HISTORY,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &Size, VoidListener),
            parser: Processor::new(),
        }
    }
}

impl Engine for Alacritty {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn screen(&self) -> Screen {
        let grid = self.term.grid();
        let spacer = Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER;
        let rows = (0..i32::from(ROWS)).map(|line| {
            let row = &grid[Line(line)];
            let mut text = String::new();
            for column in 0..usize::from(COLUMNS) {
                let cell = &row[Column(column)];
                if cell.flags.intersects(spacer) {
                    continue;
                }
                // A tab keeps its character in the cell it started from; it shows as a space.
                text.push(if cell.c == '\t' { ' ' } else { cell.c });
                text.extend(cell.zerowidth().into_iter().flatten());
            }
            text
        });
        let point = grid.cursor.point;
        let cursor = (
            u16::try_from(point.line.0).unwrap_or(0),
            u16::try_from(point.column.0).unwrap_or(0),
        );
        Screen::new(cursor, rows)
    }
}
