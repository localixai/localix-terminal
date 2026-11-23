//! The `vt100` crate: a small terminal parser and screen, the floor every core should clear.

use std::num::NonZeroU16;

use crate::{COLUMNS, Engine, HISTORY, ROWS, Screen};

pub struct Vt100 {
    parser: ::vt100::Parser,
}

impl Vt100 {
    pub fn new() -> Self {
        let (Some(rows), Some(columns)) = (NonZeroU16::new(ROWS), NonZeroU16::new(COLUMNS)) else {
            unreachable!("the bench's size is not zero");
        };
        Self {
            parser: ::vt100::Parser::new(rows, columns, HISTORY),
        }
    }
}

impl Engine for Vt100 {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    fn screen(&self) -> Screen {
        let screen = self.parser.screen();
        Screen::new(screen.cursor_position(), screen.rows(0, COLUMNS))
    }
}
