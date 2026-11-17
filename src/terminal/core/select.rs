//! Selecting text by the core's own gestures: a press, a drag and a release in the view become a
//! selection by cell, word (a double click) or line (a triple), a block with Option and Command
//! held, the view asked to scroll on while the pointer is dragged past an edge; all of it, and
//! the selection's text as it is copied (wrapped lines joined, trailing spaces left out).

use std::ptr;

use ghostty_vt_sys as vt;

use super::{Core, Refused, done};
use crate::surface::{Edge, Geometry, Mods, Mouse, MouseAction};

/// Two clicks this close in time are one double click.
const REPEAT_NS: u64 = 500_000_000;

impl Core {
    /// `mouse` as a selection gesture in a view of `geometry`: the selection it makes is the
    /// terminal's; the edge it is dragged past, if any.
    pub fn select(&mut self, mouse: &Mouse, geometry: &Geometry) -> Option<Edge> {
        let at = self.cell_under(mouse, geometry);
        self.gestured(mouse, geometry, at)
    }

    /// `mouse` as a selection gesture over the cell `at` (none: the pointer is over no cell).
    fn gestured(
        &mut self,
        mouse: &Mouse,
        geometry: &Geometry,
        at: Option<vt::GridRef>,
    ) -> Option<Edge> {
        let kind = match mouse.action {
            MouseAction::Press => vt::SelectionGestureEventType::PRESS,
            MouseAction::Motion => vt::SelectionGestureEventType::DRAG,
            MouseAction::Release => vt::SelectionGestureEventType::RELEASE,
        };
        let term = self.term;
        let (gesture, event) = self.handles.gesture(kind)?;
        // The event is kept from one gesture to the next: over no cell, the cell it was last
        // given is cleared, never read again after the page under it may have gone.
        match at {
            Some(at) => option(event, vt::SelectionGestureEventOption::REF, &at),
            // SAFETY: a null value clears the option of a live event.
            None => drop(unsafe {
                vt::ghostty_selection_gesture_event_set(
                    event,
                    vt::SelectionGestureEventOption::REF,
                    ptr::null(),
                )
            }),
        }
        let position = vt::SurfacePosition {
            x: f64::from(mouse.x),
            y: f64::from(mouse.y),
        };
        option(event, vt::SelectionGestureEventOption::POSITION, &position);
        option(
            event,
            vt::SelectionGestureEventOption::TIME_NS,
            &mouse.time_ns,
        );
        option(
            event,
            vt::SelectionGestureEventOption::REPEAT_INTERVAL_NS,
            &REPEAT_NS,
        );
        let block = mouse.mods.has(Mods::ALT | Mods::SUPER);
        option(event, vt::SelectionGestureEventOption::RECTANGLE, &block);
        let shape = vt::SelectionGestureGeometry {
            columns: u32::from(self.size().1),
            cell_width: geometry.cell_width.max(1),
            padding_left: geometry.pad_left,
            screen_height: geometry.height.max(1),
        };
        option(event, vt::SelectionGestureEventOption::GEOMETRY, &shape);
        let mut made = vt::Selection {
            start: vt::GridRef::default(),
            end: vt::GridRef::default(),
            rectangle: false,
        };
        // SAFETY: a live gesture and event over the live terminal they were used with.
        let result =
            unsafe { vt::ghostty_selection_gesture_event(gesture, term, event, &raw mut made) };
        match (result, mouse.action) {
            (vt::Result::SUCCESS, _) => self.set_selection(Some(&made)),
            // A single press clears what was selected; a drag starts the new one.
            (_, MouseAction::Press) => self.set_selection(None),
            _ => {}
        }
        let mut edge: vt::SelectionGestureAutoscroll::Type = vt::SelectionGestureAutoscroll::NONE;
        let out: *mut vt::SelectionGestureAutoscroll::Type = &raw mut edge;
        let data = vt::SelectionGestureData::AUTOSCROLL;
        // SAFETY: the gesture's autoscroll is written as the type `out` points to.
        let _ = unsafe { vt::ghostty_selection_gesture_get(gesture, term, data, out.cast()) };
        match edge {
            vt::SelectionGestureAutoscroll::UP => Some(Edge::Top),
            vt::SelectionGestureAutoscroll::DOWN => Some(Edge::Bottom),
            _ => None,
        }
    }

    /// The cell under the pointer, held to the grid.
    fn cell_under(&self, mouse: &Mouse, geometry: &Geometry) -> Option<vt::GridRef> {
        let (rows, columns) = self.size();
        let cell = |at: f32, pad: u32, size: u32, most: u16| {
            let n = ((f64::from(at) - f64::from(pad)) / f64::from(size.max(1))).floor();
            whole(n.clamp(0.0, f64::from(most)))
        };
        let column = cell(
            mouse.x,
            geometry.pad_left,
            geometry.cell_width,
            columns.saturating_sub(1),
        );
        let row = cell(
            mouse.y,
            geometry.pad_top,
            geometry.cell_height,
            rows.saturating_sub(1),
        );
        self.viewport_cell(row, column)
    }

    /// Everything selected: the history and the screen.
    pub fn select_all(&mut self) {
        let mut all = vt::Selection {
            start: vt::GridRef::default(),
            end: vt::GridRef::default(),
            rectangle: false,
        };
        // SAFETY: the selection is written on success.
        if unsafe { vt::ghostty_terminal_select_all(self.term, &raw mut all) }
            == vt::Result::SUCCESS
        {
            self.set_selection(Some(&all));
        }
    }

    /// The selection becomes `selection` (none clears it).
    pub fn set_selection(&mut self, selection: Option<&vt::Selection>) {
        let value: *const vt::Selection = selection.map_or(ptr::null(), ptr::from_ref);
        // SAFETY: the terminal copies the selection, or clears it for null.
        let _ = unsafe {
            vt::ghostty_terminal_set(self.term, vt::TerminalOption::SELECTION, value.cast())
        };
    }

    /// The selected text as it is copied: wrapped lines joined, each line's trailing spaces left
    /// out; none when nothing is selected.
    pub fn selection_text(&self) -> Option<String> {
        let mut selection = vt::Selection {
            start: vt::GridRef::default(),
            end: vt::GridRef::default(),
            rectangle: false,
        };
        let out: *mut vt::Selection = &raw mut selection;
        // SAFETY: the selection is written as the type `out` points to, when there is one.
        let read =
            unsafe { vt::ghostty_terminal_get(self.term, vt::TerminalData::SELECTION, out.cast()) };
        done(read, "the selection").ok()?;
        let text = self.copied(&selection).ok()?;
        (!text.is_empty()).then_some(text)
    }

    fn copied(&self, selection: &vt::Selection) -> Result<String, Refused> {
        let formatter = self.formatter(true)?;
        let trim = true;
        let trimmed: *const bool = &trim;
        // SAFETY: the option takes a bool, read during the call.
        let set = unsafe {
            vt::ghostty_formatter_set(formatter, vt::FormatterOption::TRIM, trimmed.cast())
        };
        let text =
            done(set, "a formatter's option").map(|()| super::text::format(formatter, selection));
        // SAFETY: made above, freed once.
        unsafe { vt::ghostty_formatter_free(formatter) };
        text
    }
}

/// `n`, whole and within a column's or row's range, as one.
fn whole(n: f64) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to the grid's columns or rows just before"
    )]
    let n = n as u16;
    n
}

fn option<T>(
    event: vt::SelectionGestureEvent,
    option: vt::SelectionGestureEventOption::Type,
    value: &T,
) {
    let value: *const T = value;
    // SAFETY: the option reads the type `value` points to during the call.
    let _ = unsafe { vt::ghostty_selection_gesture_event_set(event, option, value.cast()) };
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
