//! A terminal session's screen: the output applied as a terminal would (cursor moves, erases,
//! redraws, wide characters, the lines a box is drawn with, a resize that wraps lines again), by
//! the terminal core, for what agents read. The core gets the output whole and answers a program's
//! queries itself; the session reads beside it what the core has no callback for
//! (`screen/safe.rs`: the title, a shell's marks, notes, the program's state). While a program
//! holds a frame back as it draws it (synchronized output), the core keeps the frame's bytes
//! and its screen is the frame before: what agents read is what the view shows.

use super::core::{Core, Refused};
use super::marks::Marks;
use super::notes::Notes;
use super::status::Records;
use crate::surface::CellPixels;

mod safe;

use safe::Safe;

/// What puts right a terminal a command left wrong: no mouse reports of any kind, in any
/// encoding, and the cursor shown.
const RIGHTED: &[u8] = b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?25h";

pub(super) struct Screen {
    safe: Safe,
    core: Core,
}

impl Screen {
    /// A blank screen of `rows` by `columns`.
    ///
    /// # Errors
    /// A grid with no rows or columns.
    pub(super) fn new(rows: u16, columns: u16) -> Result<Self, Refused> {
        Ok(Self {
            safe: Safe::default(),
            core: Core::new(rows, columns)?,
        })
    }

    /// Takes in the program's output. When the shell says a command ended and runs nothing
    /// more, what that command left on (the mouse reported, the cursor hidden) is put right, or
    /// the prompt would take clicks as typing; whether it was.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> bool {
        let ended = self.safe.marks.ended().0;
        self.safe.read(bytes);
        self.core.feed(bytes);
        let left_on = !self.core.cursor_visible() || self.core.mouse_tracking();
        let marks = &self.safe.marks;
        if marks.ended().0 == ended || marks.running().is_some() || !left_on {
            return false;
        }
        self.core.feed(RIGHTED);
        true
    }

    /// The core, for a view to draw and act on.
    pub(super) fn core(&self) -> &Core {
        &self.core
    }

    pub(super) fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// The title the program set last, cleaned to plain words; `None` until it sets one.
    pub(super) fn title(&self) -> Option<&str> {
        self.safe.title.as_deref()
    }

    /// Counts the times the title changed.
    pub(super) fn retitled(&self) -> u32 {
        self.safe.retitled
    }

    /// Whether the program asked for cursor keys in application mode (DECCKM).
    pub(super) fn application_cursor(&self) -> bool {
        self.core.application_cursor()
    }

    /// What the shell said of its commands.
    pub(super) fn marks(&self) -> &Marks {
        &self.safe.marks
    }

    pub(super) fn marks_mut(&mut self) -> &mut Marks {
        &mut self.safe.marks
    }

    /// What the program asked to tell the person.
    pub(super) fn notes(&mut self) -> &mut Notes {
        &mut self.safe.notes
    }

    /// What the program said of its state.
    pub(super) fn status(&self) -> &Records {
        &self.safe.status
    }

    pub(super) fn status_mut(&mut self) -> &mut Records {
        &mut self.safe.status
    }

    /// How many times the program asked which states the terminal reads since this was last
    /// asked: the session owes each an answer.
    pub(super) fn status_asked(&mut self) -> usize {
        std::mem::take(&mut self.safe.asked)
    }

    /// Whether the program asked to be told where a paste starts and ends.
    pub(super) fn bracketed_paste(&self) -> bool {
        self.core.bracketed_paste()
    }

    /// The screen as text, its blank rows at the end left out, and where the cursor is (row and
    /// column, from 1).
    pub(super) fn text(&self) -> (String, (u16, u16)) {
        (self.core.text(), self.core.cursor())
    }

    /// The screen as text to look in: a row that wraps on to the next is joined to it with no line
    /// break and its trailing spaces kept, so a text that crosses the pane's edge is found whole.
    /// The rows are as `text` has them otherwise.
    pub(super) fn search_text(&self) -> String {
        self.core.search_text()
    }

    /// The grid's rows and columns.
    pub(super) fn size(&self) -> (u16, u16) {
        self.core.size()
    }

    /// The grid becomes `rows` by `columns` of cells `cell` in pixels, its lines wrapped again
    /// to fit and its scroll region the whole of it; a frame the program holds is let go of
    /// first. The same size in cells and pixels changes nothing, and a grid of no rows or
    /// columns is not taken.
    pub(super) fn resize(&mut self, rows: u16, columns: u16, cell: CellPixels) {
        let (now_rows, now_columns) = self.size();
        if (rows, columns, cell) != (now_rows, now_columns, self.core.cell_pixels()) {
            let _ = self.core.resize(rows, columns, cell);
        }
    }

    /// What the core answered the program's queries with since this was last asked.
    pub(super) fn replies(&mut self) -> Vec<u8> {
        self.core.replies()
    }
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "screen_drawn_tests.rs"]
mod drawn_tests;
