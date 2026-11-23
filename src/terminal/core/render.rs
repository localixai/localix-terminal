//! A terminal's frame as a view draws it, from the core's render state: the rows in view by
//! their stable ids, which changed, each changed row's cells (text, width, style, ground), the
//! selection a row holds, the cursor, the colours, and how far the view is scrolled. The update
//! is made in two parts: the first reads the terminal and needs its lock; the second, which
//! copies the rows out, only reads what the render state took, so the terminal's lock is let go
//! of between them.

use std::ffi::c_void;
use std::ptr;

use ghostty_vt_sys as vt;

use super::cell_bits;
use super::style::{raw_row, rgb};
use super::{Core, Refused, done};
use crate::surface::{Colors, Cursor, Frame, Row, RowId, Scrollbar, Shape};

/// One view's render state, and the iterators that read rows and cells out of it.
pub struct Render {
    state: vt::RenderState,
    rows: vt::RenderStateRowIterator,
    cells: vt::RenderStateRowCells,
    /// What the first part read of the terminal itself, for the second to put in the frame.
    scroll: Scrollbar,
    alternate: bool,
    mouse: bool,
    /// What the last frame said besides its rows. The library's dirt covers rows alone, so a
    /// cursor that only moved (a shell's Left arrow, a backspace), changed its shape or hid, a
    /// colour a program set, or a mode turned on, is a new frame by this.
    told: Option<Told>,
}

/// What a frame says that is not a row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Told {
    cursor: Cursor,
    colors: Colors,
    scroll: Scrollbar,
    alternate: bool,
    mouse: bool,
}

// SAFETY: the render state and its iterators belong to this handle alone, are bound to no
// terminal, and the library keeps no thread of its own for them; one thread calls at a time.
unsafe impl Send for Render {}

impl Render {
    /// A render state with nothing in it yet: its first update is a full frame.
    ///
    /// # Errors
    /// The library could not make one.
    pub fn new() -> Result<Self, Refused> {
        let mut render = Self {
            state: ptr::null_mut(),
            rows: ptr::null_mut(),
            cells: ptr::null_mut(),
            scroll: Scrollbar::default(),
            alternate: false,
            mouse: false,
            told: None,
        };
        // SAFETY: a null allocator is the library's default; each handle is written on success
        // and freed by `drop`, which skips one left null.
        let made = unsafe { vt::ghostty_render_state_new(ptr::null(), &raw mut render.state) };
        done(made, "to make a render state")?;
        // SAFETY: as above.
        let made =
            unsafe { vt::ghostty_render_state_row_iterator_new(ptr::null(), &raw mut render.rows) };
        done(made, "to make a row iterator")?;
        // SAFETY: as above.
        let made =
            unsafe { vt::ghostty_render_state_row_cells_new(ptr::null(), &raw mut render.cells) };
        done(made, "to make a cell iterator")?;
        Ok(render)
    }

    /// The first part of an update, with the terminal's lock held: the render state takes what
    /// it needs of the terminal, and the scrollbar and modes are read at the same moment.
    ///
    /// # Errors
    /// The library refused the update.
    pub fn begin(&mut self, core: &Core) -> Result<(), Refused> {
        // SAFETY: both are live; the terminal is not changed during the call (the caller holds
        // the lock it is changed under).
        let begun = unsafe { vt::ghostty_render_state_begin_update(self.state, core.term) };
        done(begun, "an update")?;
        self.read_terminal(core.term);
        Ok(())
    }

    /// The scrollbar and modes of `term`, read with the update.
    fn read_terminal(&mut self, term: vt::Terminal) {
        let bar: vt::TerminalScrollbar = super::get_of(term, vt::TerminalData::SCROLLBAR);
        self.scroll = Scrollbar {
            total: bar.total,
            offset: bar.offset,
            len: bar.len,
        };
        let screen: vt::TerminalScreen::Type = super::get_of(term, vt::TerminalData::ACTIVE_SCREEN);
        self.alternate = screen == vt::TerminalScreen::ALTERNATE;
        self.mouse = super::get_of(term, vt::TerminalData::MOUSE_TRACKING);
    }

    /// The second part, the terminal's lock let go of: what changed since the last frame (every
    /// row when `full`) is copied into `into`, and the render state is clean again. `false` when
    /// nothing changed.
    pub fn finish(&mut self, into: &mut Frame, full: bool) -> bool {
        // SAFETY: only the render state's own memory is touched; an end with no begin is a no-op.
        let _ = unsafe { vt::ghostty_render_state_end_update(self.state) };
        let dirty: vt::RenderStateDirty::Type = self.get(vt::RenderStateData::DIRTY);
        let mut colors = into.colors.clone();
        self.colors(&mut colors);
        let told = Told {
            cursor: self.cursor(),
            colors,
            scroll: self.scroll,
            alternate: self.alternate,
            mouse: self.mouse,
        };
        if dirty == vt::RenderStateDirty::FALSE && !full && self.told.as_ref() == Some(&told) {
            return false;
        }
        into.full = full || dirty == vt::RenderStateDirty::FULL;
        into.columns = self.get(vt::RenderStateData::COLS);
        into.lines = self.get(vt::RenderStateData::ROWS);
        into.scroll = told.scroll;
        into.alternate = told.alternate;
        into.mouse_taken = told.mouse;
        into.colors.clone_from(&told.colors);
        into.cursor = told.cursor;
        let cursor_row = told.cursor.at.map(|(_, y)| y);
        self.told = Some(told);
        self.rows_into(into, cursor_row);
        // SAFETY: the frame was read whole; both layers of dirt are cleared.
        let _ = unsafe { vt::ghostty_render_state_clean(self.state) };
        true
    }

    /// How far the view was scrolled when the last update began.
    pub fn scroll(&self) -> Scrollbar {
        self.scroll
    }

    /// Both parts at once, for a caller that holds the core alone.
    pub fn frame(&mut self, core: &Core, into: &mut Frame, full: bool) -> bool {
        self.begin(core).is_ok() && self.finish(into, full)
    }

    /// The render state's `data`, as the type the library writes for it; the default where it
    /// has none.
    fn get<T: Default>(&self, data: vt::RenderStateData::Type) -> T {
        let mut value = T::default();
        let out: *mut T = &raw mut value;
        // SAFETY: `out` points to the type `data` is written as, alive through the call.
        let _ = unsafe { vt::ghostty_render_state_get(self.state, data, out.cast::<c_void>()) };
        value
    }

    fn colors(&self, into: &mut Colors) {
        let mut colors = vt::sized!(vt::RenderStateColors);
        let out: *mut vt::RenderStateColors = &raw mut colors;
        let data = vt::RenderStateData::COLORS;
        // SAFETY: a sized struct of the type the colours are written as.
        let read = unsafe { vt::ghostty_render_state_get(self.state, data, out.cast()) };
        if done(read, "the colours").is_err() {
            return;
        }
        into.foreground = rgb(colors.foreground);
        into.background = rgb(colors.background);
        into.cursor = colors.cursor_has_value.then(|| rgb(colors.cursor));
        for (to, from) in into.palette.iter_mut().zip(colors.palette) {
            *to = rgb(from);
        }
    }

    fn cursor(&self) -> Cursor {
        let mut cursor = vt::sized!(vt::RenderStateCursor);
        let out: *mut vt::RenderStateCursor = &raw mut cursor;
        let data = vt::RenderStateData::CURSOR;
        // SAFETY: a sized struct of the type the cursor is written as.
        let read = unsafe { vt::ghostty_render_state_get(self.state, data, out.cast()) };
        if done(read, "the cursor").is_err() {
            return Cursor::default();
        }
        let shown = cursor.visible && cursor.viewport_has_value;
        Cursor {
            at: shown.then_some((cursor.viewport_x, cursor.viewport_y)),
            shape: match cursor.visual_style {
                vt::CursorStyle::BAR => Shape::Bar,
                vt::CursorStyle::UNDERLINE => Shape::Underline,
                vt::CursorStyle::BLOCK_HOLLOW => Shape::Hollow,
                _ => Shape::Block,
            },
            blinking: cursor.blinking,
            wide_tail: cursor.wide_tail,
        }
    }

    /// Every row in view into `into.rows`, reusing what they held; a row's cells only when it is
    /// dirty or the frame is full, and the cursor's row (`cursor_row`) always: a block cursor
    /// draws the character it stands on, and a cursor that only moved leaves its row clean.
    fn rows_into(&mut self, into: &mut Frame, cursor_row: Option<u16>) {
        let out: *mut vt::RenderStateRowIterator = &raw mut self.rows;
        let data = vt::RenderStateData::ROW_ITERATOR;
        // SAFETY: the iterator made in `new` is pointed at this render state's rows.
        let read = unsafe { vt::ghostty_render_state_get(self.state, data, out.cast()) };
        let mut count = 0;
        let next = |rows| {
            // SAFETY: the iterator was set just now, over a render state not updated since.
            unsafe { vt::ghostty_render_state_row_iterator_next(rows) }
        };
        while done(read, "the rows").is_ok() && next(self.rows) {
            if into.rows.len() == count {
                into.rows.push(Row::default());
            }
            if let Some(row) = into.rows.get_mut(count) {
                self.row_into(row, into.full, cursor_row);
            }
            count += 1;
        }
        into.rows.truncate(count);
    }

    fn row_get<T: Default>(&self, data: vt::RenderStateRowData::Type) -> Option<T> {
        let mut value = T::default();
        let out: *mut T = &raw mut value;
        // SAFETY: `out` points to the type `data` is written as; the iterator is at a row.
        let read = unsafe { vt::ghostty_render_state_row_get(self.rows, data, out.cast()) };
        done(read, "a row").ok().map(|()| value)
    }

    fn row_into(&mut self, row: &mut Row, full: bool, cursor_row: Option<u16>) {
        let id: vt::RenderStateRowId = self.row_get(vt::RenderStateRowData::ID).unwrap_or_default();
        let [high, low] = id.bits;
        row.id = RowId(u128::from(high) << 64 | u128::from(low));
        let y: i32 = self
            .row_get(vt::RenderStateRowData::VIEWPORT_Y)
            .unwrap_or_default();
        row.y = u16::try_from(y).unwrap_or_default();
        row.dirty = full || self.row_get(vt::RenderStateRowData::DIRTY).unwrap_or(true);
        let raw: vt::Row = self
            .row_get(vt::RenderStateRowData::RAW)
            .unwrap_or_default();
        row.wrapped = raw_row(raw, vt::RowData::WRAP);
        let mut selection = vt::sized!(vt::RenderStateRowSelection);
        let out: *mut vt::RenderStateRowSelection = &raw mut selection;
        let data = vt::RenderStateRowData::SELECTION;
        // SAFETY: a sized struct of the type a row's selection is written as.
        let read = unsafe { vt::ghostty_render_state_row_get(self.rows, data, out.cast()) };
        row.selected =
            (read == vt::Result::SUCCESS).then_some((selection.start_x, selection.end_x));
        row.text.clear();
        row.cells.clear();
        if row.dirty || cursor_row == Some(row.y) {
            self.cells_into(row);
        }
    }

    /// The cells of the row the iterator is at: their raw values in one read, and the cell
    /// iterator put over the row for the cells that must be asked more.
    fn cells_into(&mut self, row: &mut Row) {
        let out: *mut vt::RenderStateRowCells = &raw mut self.cells;
        let data = vt::RenderStateRowData::CELLS;
        // SAFETY: the cell iterator made in `new` is pointed at the current row's cells.
        let read = unsafe { vt::ghostty_render_state_row_get(self.rows, data, out.cast()) };
        let raw: Option<vt::CellsView> = self.row_get(vt::RenderStateRowData::CELLS_RAW);
        let (Ok(()), Some(raw)) = (done(read, "a row's cells"), raw) else {
            return;
        };
        if raw.ptr.is_null() {
            return;
        }
        // SAFETY: the library says `ptr` holds `len` cells, valid until the render state is next
        // updated, which is after this frame is read.
        let values = unsafe { std::slice::from_raw_parts(raw.ptr, raw.len) };
        let places = cell_bits::places();
        for (x, &value) in (0..=u16::MAX).zip(values) {
            let fields = cell_bits::fields(places, value);
            let cell = super::cell::read(self.cells, fields, x, &mut row.text);
            row.cells.push(cell);
        }
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        if !self.cells.is_null() {
            // SAFETY: made in `new`, freed once.
            unsafe { vt::ghostty_render_state_row_cells_free(self.cells) };
        }
        if !self.rows.is_null() {
            // SAFETY: made in `new`, freed once.
            unsafe { vt::ghostty_render_state_row_iterator_free(self.rows) };
        }
        if !self.state.is_null() {
            // SAFETY: made in `new`, freed once, after the iterators that read it.
            unsafe { vt::ghostty_render_state_free(self.state) };
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "render_told_tests.rs"]
mod told_tests;

#[cfg(test)]
#[path = "render_cells_tests.rs"]
mod cells_tests;
