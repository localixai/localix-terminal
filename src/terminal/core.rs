//! The terminal core: libghostty-vt, a terminal's screen and history as a terminal draws them
//! (wide characters, line drawing, the other screen, a resize that wraps lines again), linked
//! statically through its raw bindings and held behind a handle the session owns. It is fed the
//! program's output and read as text (`core/text.rs`, by the core's own formatter) and for what
//! a shell said of its commands (`core/marks.rs`, OSC 133). It gets the program's output whole,
//! but for a frame the program holds back, kept until it lets go (`core/frames.rs`), and
//! answers its queries itself (`core/effects.rs`), into a queue the session writes to the
//! program; a program's copy and its bell are only recorded, for the window to decide on
//! (`core/clip.rs`), and its title is the session's filter's, never the core's.
//!
//! These files are the only ones that name the crate: the rest of the tree holds a `Core` and
//! never the library's own types, so the library's C ABI stays behind one wall.

use std::ffi::c_void;
use std::fmt;
use std::ptr;

use ghostty_vt_sys as vt;

use crate::surface::CellPixels;

mod cell;
mod cell_bits;
mod clip;
mod effects;
mod find;
mod frames;
mod handles;
mod keys;
mod links;
mod marks;
mod mouse;
mod render;
mod scroll;
mod select;
mod style;
mod text;

pub use marks::{Content, Step};
pub use render::Render;

/// The most rows of history a terminal keeps above its screen, about: the core lets rows go a
/// page at a time, so it keeps within a page of this.
const HISTORY_LINES: usize = 10_000;
/// The most bytes history takes, whichever limit comes first: at 120 columns 10,000 rows take
/// about 12 MB, and a far wider terminal reaches this before its rows run out.
const HISTORY_BYTES: usize = 16 << 20;
/// The mode in which a character of several codepoints is measured as Unicode says.
const GRAPHEMES: u16 = 2027;

/// One terminal's screen and history, and the parser that reads a program's output into them.
pub struct Core {
    term: vt::Terminal,
    /// The core's formatters over `term`: one reads the rows as they are, one joins a line that
    /// wrapped. Each borrows the terminal, so each is freed first.
    plain: vt::Formatter,
    joined: vt::Formatter,
    /// What the core's callbacks heard (its answers, a shell's steps, a copy, a bell): made as
    /// the core is (`Box::into_raw`) and freed after its terminal, so no callback outlives it.
    heard: *const effects::Heard,
    /// The encoders and the selection gesture, made as a view first needs them.
    handles: handles::Handles,
    /// What a find looks for and found.
    found: find::Found,
    /// The bytes of a frame the program holds back, not yet written.
    kept: frames::Kept,
}

// SAFETY: the terminal, its formatters and the callback's state belong to this handle alone, and
// the library keeps no thread of its own or thread-local state for them: moving the handle to
// another thread moves everything they reach. `Core` is not `Sync`, so one thread calls at a time.
unsafe impl Send for Core {}

/// What the core cannot do: a size with no rows or no columns, or the library's own refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    what: &'static str,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the terminal core refused {}", self.what)
    }
}

impl std::error::Error for Refused {}

/// `Ok` when the library said it succeeded, else `what` was refused.
fn done(result: vt::Result::Type, what: &'static str) -> Result<(), Refused> {
    match result {
        vt::Result::SUCCESS => Ok(()),
        _ => Err(Refused { what }),
    }
}

/// The size `rows` by `columns`, unless it has no rows or no columns.
fn sized(what: &'static str, rows: u16, columns: u16) -> Result<(u16, u16), Refused> {
    match rows == 0 || columns == 0 {
        true => Err(Refused { what }),
        false => Ok((rows, columns)),
    }
}

impl Core {
    /// A blank terminal of `rows` by `columns`, its history held to `HISTORY_LINES` and
    /// `HISTORY_BYTES`.
    ///
    /// # Errors
    /// A grid with no rows or columns, or a terminal the library could not make.
    pub fn new(rows: u16, columns: u16) -> Result<Self, Refused> {
        let (rows, columns) = sized("a grid of no rows or no columns", rows, columns)?;
        let mut term: vt::Terminal = ptr::null_mut();
        // SAFETY: a null allocator is the library's default; `term` is written on success.
        let made = unsafe { vt::ghostty_terminal_new(ptr::null(), &raw mut term, columns, rows) };
        done(made, "to make a terminal")?;
        let mut core = Self {
            term,
            plain: ptr::null_mut(),
            joined: ptr::null_mut(),
            heard: Box::into_raw(Box::<effects::Heard>::default()),
            handles: handles::Handles::default(),
            found: find::Found::default(),
            kept: frames::Kept::default(),
        };
        core.set(vt::TerminalOption::SCROLLBACK_MAX_LINES, &HISTORY_LINES)?;
        core.set(vt::TerminalOption::SCROLLBACK_MAX_BYTES, &HISTORY_BYTES)?;
        // Characters of several codepoints take the columns Unicode gives them (mode 2027), as
        // the programs that draw beside them count them; it is the mode a reset comes back to,
        // and a program may still turn it off.
        let unicode = vt::TerminalModeConfig {
            mode: GRAPHEMES,
            value: true,
        };
        core.set(vt::TerminalOption::MODE_DEFAULT, &unicode)?;
        core.plain = core.formatter(false)?;
        core.joined = core.formatter(true)?;
        effects::listen(&core)?;
        clip::listen(&core)?;
        marks::listen(&core)?;
        Ok(core)
    }

    /// Sets the terminal's `option` to what `value` points to.
    fn set<T>(&self, option: vt::TerminalOption::Type, value: &T) -> Result<(), Refused> {
        let value: *const T = value;
        // SAFETY: `value` points to the type `option` takes, and is read only during the call.
        let set = unsafe { vt::ghostty_terminal_set(self.term, option, value.cast()) };
        done(set, "an option")
    }

    /// Reads the terminal's `data` as a `T`, the type the library writes for it; the default
    /// where it has none.
    fn get<T: Default>(&self, data: vt::TerminalData::Type) -> T {
        get_of(self.term, data)
    }

    /// Whether DEC private mode `number` is set.
    fn mode(&self, number: u16) -> bool {
        // A mode is its number, with the top bit set for an ANSI mode: a private one has it clear.
        let mut config = vt::TerminalModeConfig {
            mode: number & 0x7fff,
            value: false,
        };
        let out: *mut vt::TerminalModeConfig = &raw mut config;
        // SAFETY: the mode's query reads `mode` and writes `value` of the struct `out` points to.
        let _ = unsafe { vt::ghostty_terminal_get(self.term, vt::TerminalData::MODE, out.cast()) };
        config.value
    }

    /// The grid becomes `rows` by `columns` of cells `cell` in pixels (zero where no view drew
    /// it), which a program that asks the size is told; the main screen's lines are wrapped
    /// again to fit, rows come back from history as it grows, and the scroll region is the whole
    /// of it again. A frame the program holds is let go of first: it was drawn for the size
    /// before.
    ///
    /// # Errors
    /// A grid with no rows or columns, which leaves the size as it was.
    pub fn resize(&mut self, rows: u16, columns: u16, cell: CellPixels) -> Result<(), Refused> {
        let (rows, columns) = sized("a resize to no rows or no columns", rows, columns)?;
        self.let_go();
        let (width, height) = (cell.width, cell.height);
        // SAFETY: a resize of a live terminal.
        let resized =
            unsafe { vt::ghostty_terminal_resize(self.term, columns, rows, width, height) };
        done(resized, "a resize")
    }

    /// A cell's size in pixels, as the last resize gave it; zero before one gave it.
    pub fn cell_pixels(&self) -> CellPixels {
        cell_of(self.term)
    }

    /// The grid's rows and columns.
    pub fn size(&self) -> (u16, u16) {
        (
            self.get(vt::TerminalData::ROWS),
            self.get(vt::TerminalData::COLS),
        )
    }

    /// Where the cursor is: row and column, from 1.
    pub fn cursor(&self) -> (u16, u16) {
        cursor_of(self.term)
    }

    /// Whether a program draws in the other screen (the one full-screen programs use).
    pub fn alternate(&self) -> bool {
        let screen: vt::TerminalScreen::Type = self.get(vt::TerminalData::ACTIVE_SCREEN);
        screen == vt::TerminalScreen::ALTERNATE
    }

    /// Whether the program asked for cursor keys in application mode (DECCKM).
    pub fn application_cursor(&self) -> bool {
        self.mode(1)
    }

    /// Whether the program asked to be told where a paste starts and ends.
    pub fn bracketed_paste(&self) -> bool {
        self.mode(2004)
    }

    /// Whether the cursor is shown.
    pub fn cursor_visible(&self) -> bool {
        self.get(vt::TerminalData::CURSOR_VISIBLE)
    }

    /// Whether the program asked for mouse reports, of any kind.
    pub fn mouse_tracking(&self) -> bool {
        self.get(vt::TerminalData::MOUSE_TRACKING)
    }

    /// Whether lines that reach the right edge go on to the next row (DECAWM).
    pub fn wraps(&self) -> bool {
        self.mode(7)
    }

    /// Whether the cursor's rows count from the scroll region's top (DECOM).
    pub fn origin(&self) -> bool {
        self.mode(6)
    }

    /// The theme the terminal draws in: its text, ground and cursor, and the first sixteen
    /// colours of its palette (the rest are the core's). What a program asks of its colours is
    /// answered from these, and what it sets goes over them.
    pub fn set_colors(&mut self, palette: &crate::surface::Palette) {
        let rgb = |c: crate::surface::Rgb| vt::ColorRgb {
            r: c.r,
            g: c.g,
            b: c.b,
        };
        let mut colors = [vt::ColorRgb::default(); 256];
        // SAFETY: a palette of 256 entries is written whole, as the option reads it.
        unsafe { vt::ghostty_color_palette_default(colors.as_mut_ptr()) };
        for (to, from) in colors.iter_mut().zip(palette.ansi) {
            *to = rgb(from);
        }
        let _ = self.set(
            vt::TerminalOption::COLOR_FOREGROUND,
            &rgb(palette.foreground),
        );
        let _ = self.set(
            vt::TerminalOption::COLOR_BACKGROUND,
            &rgb(palette.background),
        );
        let _ = self.set(vt::TerminalOption::COLOR_CURSOR, &rgb(palette.cursor));
        let _ = self.set(vt::TerminalOption::COLOR_PALETTE, &colors);
    }

    /// How many rows of history are kept above the screen.
    pub fn history(&self) -> usize {
        self.get(vt::TerminalData::SCROLLBACK_ROWS)
    }
}

/// Reads `term`'s `data` as a `T`, the type the library writes for it; the default where it has
/// none. The terminal is live and not changed during the call (a callback may read it so too).
fn get_of<T: Default>(term: vt::Terminal, data: vt::TerminalData::Type) -> T {
    let mut value = T::default();
    let out: *mut T = &raw mut value;
    // SAFETY: `out` points to the type `data` is written as, alive through the call.
    let _ = unsafe { vt::ghostty_terminal_get(term, data, out.cast::<c_void>()) };
    value
}

/// `term`'s cell in pixels, as its last resize gave it: the library keeps the grid's whole width
/// and height, columns and rows times the cell's.
fn cell_of(term: vt::Terminal) -> CellPixels {
    let rows: u16 = get_of(term, vt::TerminalData::ROWS);
    let columns: u16 = get_of(term, vt::TerminalData::COLS);
    let width: u32 = get_of(term, vt::TerminalData::WIDTH_PX);
    let height: u32 = get_of(term, vt::TerminalData::HEIGHT_PX);
    CellPixels {
        width: width / u32::from(columns.max(1)),
        height: height / u32::from(rows.max(1)),
    }
}

/// Where `term`'s cursor is: row and column, from 1.
fn cursor_of(term: vt::Terminal) -> (u16, u16) {
    let row: u16 = get_of(term, vt::TerminalData::CURSOR_Y);
    let column: u16 = get_of(term, vt::TerminalData::CURSOR_X);
    (row.saturating_add(1), column.saturating_add(1))
}

impl Drop for Core {
    fn drop(&mut self) {
        self.handles.free(self.term);
        for formatter in [self.plain, self.joined] {
            if !formatter.is_null() {
                // SAFETY: made by `formatter` over this terminal, freed once, before it.
                unsafe { vt::ghostty_formatter_free(formatter) };
            }
        }
        // SAFETY: made in `new`, freed once; nothing reaches it after.
        unsafe { vt::ghostty_terminal_free(self.term) };
        // SAFETY: made by `Box::into_raw` in `new`; the terminal that called back into it is gone.
        drop(unsafe { Box::from_raw(self.heard.cast_mut()) });
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
