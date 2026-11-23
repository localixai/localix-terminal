//! wezterm's terminal core (`wezterm-term`, from its repository at the commit the manifest pins)
//! with its own parser. What it would write back to the program is dropped.

use std::sync::Arc;

use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

use crate::{COLUMNS, Engine, HISTORY, ROWS, Screen};

/// The core's settings: its defaults, with the bench's history.
#[derive(Debug)]
struct Settings;

impl TerminalConfiguration for Settings {
    fn scrollback_size(&self) -> usize {
        HISTORY
    }

    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

pub struct Wezterm {
    term: Terminal,
}

impl Wezterm {
    pub fn new() -> Self {
        let size = TerminalSize {
            rows: usize::from(ROWS),
            cols: usize::from(COLUMNS),
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        };
        let term = Terminal::new(
            size,
            Arc::new(Settings),
            "bench",
            "0",
            Box::new(std::io::sink()),
        );
        Self { term }
    }
}

impl Engine for Wezterm {
    fn feed(&mut self, bytes: &[u8]) {
        self.term.advance_bytes(bytes);
    }

    fn screen(&self) -> Screen {
        let screen = self.term.screen();
        let rows = screen.lines_in_phys_range(screen.phys_range(&(0..i64::from(ROWS))));
        let at = self.term.cursor_pos();
        let cursor = (
            u16::try_from(at.y).unwrap_or(0),
            u16::try_from(at.x).unwrap_or(0),
        );
        Screen::new(cursor, rows.iter().map(|line| line.as_str().into_owned()))
    }
}
