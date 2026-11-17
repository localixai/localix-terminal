//! The core's screen read as text by its own formatter: the rows in view a line each (each wide
//! character and each character with what combines with it once, the lines a box is drawn with
//! as the box characters they show, trailing spaces and the blank rows at the end left out); and
//! the same with a line that wrapped joined whole, to look in; and the link a cell is part of
//! (OSC 8).

use std::ptr;

use ghostty_vt_sys as vt;

use super::{Core, Refused, done};

impl Core {
    /// A formatter of plain text over this terminal, trimmed; `unwrap`, a line that wrapped comes
    /// out whole.
    pub(super) fn formatter(&self, unwrap: bool) -> Result<vt::Formatter, Refused> {
        let mut formatter: vt::Formatter = ptr::null_mut();
        // SAFETY: a null allocator is the library's default; `formatter` is written on success.
        let made = unsafe {
            vt::ghostty_formatter_terminal_new(ptr::null(), &raw mut formatter, self.term)
        };
        done(made, "to make a formatter")?;
        let value: *const bool = &unwrap;
        // SAFETY: the option takes a bool, read during the call.
        let set = unsafe {
            vt::ghostty_formatter_set(formatter, vt::FormatterOption::UNWRAP, value.cast())
        };
        if let Err(refused) = done(set, "a formatter's option") {
            // SAFETY: made above, never handed out.
            unsafe { vt::ghostty_formatter_free(formatter) };
            return Err(refused);
        }
        Ok(formatter)
    }

    /// The rows in view as plain text, a line each: trailing spaces and the blank rows at the end
    /// left out, as the screen shows them (a line that wrapped is a line a row).
    pub fn text(&self) -> String {
        self.read(self.plain)
    }

    /// The rows in view as text to look in: a row that wraps on to the next is joined to it with
    /// no line break and its spaces kept, so a text that crosses the pane's edge is found whole;
    /// each line's trailing spaces and the blank lines at the end left out.
    pub fn search_text(&self) -> String {
        self.read(self.joined)
    }

    fn read(&self, formatter: vt::Formatter) -> String {
        view_text(self.term, formatter)
    }

    /// The link (OSC 8) the cell at `row` and `column` of the rows in view is part of, from 0.
    pub fn hyperlink_at(&self, row: u16, column: u16) -> Option<String> {
        let at = self.viewport_cell(row, column)?;
        let mut needed = 0usize;
        // SAFETY: with no buffer the call only says how much room the address takes.
        let _ = unsafe {
            vt::ghostty_grid_ref_hyperlink_uri(&raw const at, ptr::null_mut(), 0, &raw mut needed)
        };
        if needed == 0 {
            return None;
        }
        let mut uri = vec![0u8; needed];
        let mut written = 0usize;
        // SAFETY: `uri` has the room asked for; the reference was read just now.
        let read = unsafe {
            vt::ghostty_grid_ref_hyperlink_uri(
                &raw const at,
                uri.as_mut_ptr(),
                needed,
                &raw mut written,
            )
        };
        done(read, "a link").ok()?;
        uri.truncate(written);
        String::from_utf8(uri).ok().filter(|uri| !uri.is_empty())
    }

    /// The whole history and screen, `total` rows, as plain text a row a line (blank rows as
    /// empty lines), for a find to look in.
    pub(super) fn screen_text(&self, total: usize) -> String {
        let (last_row, last_column) = (total.saturating_sub(1), self.size().1.saturating_sub(1));
        let last_row = u32::try_from(last_row).unwrap_or(u32::MAX);
        let (Some(start), Some(end)) = (
            self.screen_cell(0, 0),
            self.screen_cell(last_row, last_column),
        ) else {
            return String::new();
        };
        let all = vt::Selection {
            start,
            end,
            rectangle: false,
        };
        format(self.plain, &all)
    }

    /// The cell at `row` and `column` of the rows in view, from 0.
    pub(super) fn cell(&self, row: u16, column: u16) -> Option<vt::GridRef> {
        self.grid_ref(vt::PointTag::ACTIVE, u32::from(row), column)
    }

    /// The cell at `row` and `column` of what the view shows (scrolled into history or not).
    pub(super) fn viewport_cell(&self, row: u16, column: u16) -> Option<vt::GridRef> {
        self.grid_ref(vt::PointTag::VIEWPORT, u32::from(row), column)
    }

    /// The cell at `row` and `column` of the history and screen together, from the top.
    pub(super) fn screen_cell(&self, row: u32, column: u16) -> Option<vt::GridRef> {
        self.grid_ref(vt::PointTag::SCREEN, row, column)
    }

    fn grid_ref(&self, tag: vt::PointTag::Type, row: u32, column: u16) -> Option<vt::GridRef> {
        grid_ref_of(self.term, tag, row, column)
    }
}

/// The cell at `row` and `column` of `term`, counted as `tag` says.
fn grid_ref_of(
    term: vt::Terminal,
    tag: vt::PointTag::Type,
    row: u32,
    column: u16,
) -> Option<vt::GridRef> {
    let point = vt::Point {
        tag,
        value: vt::PointValue {
            coordinate: vt::PointCoordinate { x: column, y: row },
        },
    };
    let mut cell = vt::GridRef::default();
    // SAFETY: `cell` is written on success; the reference is read before the terminal changes.
    let found = unsafe { vt::ghostty_terminal_grid_ref(term, point, &raw mut cell) };
    done(found, "a cell").ok().map(|()| cell)
}

/// What `formatter` (made over `term`) makes of the rows in view, trailing line breaks and
/// spaces left out; nothing where the library refuses. It only reads the terminal, so a callback
/// may call it as the core writes.
pub(super) fn view_text(term: vt::Terminal, formatter: vt::Formatter) -> String {
    let rows: u16 = super::get_of(term, vt::TerminalData::ROWS);
    let columns: u16 = super::get_of(term, vt::TerminalData::COLS);
    let (Some(last_row), Some(last_column)) = (rows.checked_sub(1), columns.checked_sub(1)) else {
        return String::new();
    };
    let cell = |row: u16, column| grid_ref_of(term, vt::PointTag::ACTIVE, u32::from(row), column);
    let (Some(start), Some(end)) = (cell(0, 0), cell(last_row, last_column)) else {
        return String::new();
    };
    let view = vt::Selection {
        start,
        end,
        rectangle: false,
    };
    let text = format(formatter, &view);
    text.trim_end_matches(['\n', ' ']).to_string()
}

/// What `formatter` makes of `selection`; nothing where the library refuses.
pub(super) fn format(formatter: vt::Formatter, selection: &vt::Selection) -> String {
    let value: *const vt::Selection = selection;
    // SAFETY: the selection is copied by the formatter, and is of this terminal as it is now.
    let set = unsafe {
        vt::ghostty_formatter_set(formatter, vt::FormatterOption::SELECTION, value.cast())
    };
    if done(set, "a selection").is_err() {
        return String::new();
    }
    let mut needed = 0usize;
    // SAFETY: with no buffer the call only says how much room the text takes.
    let _ =
        unsafe { vt::ghostty_formatter_format_buf(formatter, ptr::null_mut(), 0, &raw mut needed) };
    let mut text = vec![0u8; needed];
    let mut written = 0usize;
    // SAFETY: `text` has the room the library asked for, and lives through the call.
    let formatted = unsafe {
        vt::ghostty_formatter_format_buf(formatter, text.as_mut_ptr(), needed, &raw mut written)
    };
    if done(formatted, "to format the screen").is_err() {
        return String::new();
    }
    text.truncate(written);
    String::from_utf8_lossy(&text).into_owned()
}
