//! What the pointer finds over the view, as the terminal page had it. Links: while Super is held,
//! the link under the pointer (one its program gave, or an http or https address in its line) is
//! underlined and the pointer is a hand; a Super-click opens it where the window says
//! (`term_links.rs`: in a tab, in the default browser, or asked). The
//! scrollbar over the right edge: its slider shows only while there is history to scroll and the
//! pointer is over the view; pointed at it is brighter; dragged, the view follows it, and a press
//! in its track brings the slider there. And the pointer's shape over the view.

use gtk::gdk;
use gtk::prelude::WidgetExt;

use super::view::View;
use super::{acts, overlay};
use crate::surface::{Link, Mods, ScrollTo, Scrollbar};

/// The cell under `at` (the view's points from its top left), as the grid is drawn now.
fn cell(view: &View, at: (f64, f64)) -> Option<(u16, u16)> {
    let state = view.state()?;
    acts::cell_at(&state.frame, state.painter.as_ref()?.cell(), at)
}

/// The link under `at` while Super is held, as the terminal says it.
fn under(view: &View, at: (f64, f64), command: bool) -> Option<Link> {
    let surface = view.surface()?;
    let (column, y) = cell(view, at).filter(|_| command)?;
    surface.link_at(column, y)
}

/// The pointer is at `at` with `mods` held: the link under it underlined while Super is, and
/// the pointer's shape for it.
pub(super) fn hover(view: &View, at: (f64, f64), mods: Mods) {
    let link = under(view, at, mods.has(Mods::SUPER));
    let changed = view.state().is_some_and(|mut state| {
        state.pointer = Some(at);
        let changed = state.link != link;
        if changed {
            state.frame.link = (link.as_ref())
                .map(|link| link.spans.clone())
                .unwrap_or_default();
            state.link = link;
        }
        changed
    });
    if changed {
        view.redraw(true);
    }
    shape(view);
}

/// Super pressed or let go where the pointer last was: the link under it lit or not.
pub(super) fn mods(view: &View, mods: Mods) {
    if let Some(at) = view.state().and_then(|state| state.pointer) {
        hover(view, at, mods);
    }
}

/// A press at `at` with `mods` held: the link under it opened when Super is; whether it was.
pub(super) fn open(view: &View, at: (f64, f64), mods: Mods) -> bool {
    if !mods.has(Mods::SUPER) || mods.has(Mods::ALT) {
        return false;
    }
    let Some(link) = under(view, at, true) else {
        return false;
    };
    if let Some(state) = view.state() {
        (state.opens)(&link.uri);
    }
    true
}

/// The pointer's shape over the view: a hand on a link lit, the arrow while the program takes
/// the mouse, else the I-beam of text.
fn shape(view: &View) {
    let (link, taken) = view.state().map_or((false, false), |state| {
        (state.link.is_some(), state.frame.mouse_taken)
    });
    let name = match (link, taken) {
        (true, _) => "pointer",
        (false, true) => "default",
        (false, false) => "text",
    };
    if let Some(window) = view.area.window() {
        let cursor = gdk::Cursor::from_name(&view.area.display(), name);
        window.set_cursor(cursor.as_ref());
    }
}

/// The pointer came over the view (`inside`) or left it: the slider shows or goes.
pub(super) fn pointer_in(view: &View, inside: bool) {
    let changed = view.state().is_some_and(|mut state| {
        let changed = state.bar_shown != inside;
        state.bar_shown = inside;
        if !inside {
            state.bar_pointed = false;
            state.pointer = None;
        }
        changed
    });
    if changed {
        view.redraw(false);
    }
}

/// The view's size in points.
fn size(view: &View) -> (f64, f64) {
    let area = &view.area;
    (
        f64::from(area.allocated_width()),
        f64::from(area.allocated_height()),
    )
}

/// The slider's top and length, the view's height, and the rows it stands for.
fn slider(view: &View) -> Option<(f64, f64, f64, Scrollbar)> {
    let (_, height) = size(view);
    let state = view.state()?;
    let (top, length) = overlay::scrollbar(&state.frame, height)?;
    Some((top, length, height, state.frame.scroll))
}

/// Whether `at` (the view's points) is over the scrollbar's column while it shows.
fn over(view: &View, (x, _): (f64, f64)) -> bool {
    let shown = view.state().is_some_and(|state| state.bar_shown);
    shown && acts::on_bar(x, size(view).0) && slider(view).is_some()
}

/// The pointer moved to `at`: the slider brighter while it is on it.
pub(super) fn moved(view: &View, at: (f64, f64)) {
    let on = over(view, at)
        && slider(view).is_some_and(|(top, length, ..)| (top..top + length).contains(&at.1));
    let changed = view.state().is_some_and(|mut state| {
        let changed = state.bar_pointed != on;
        state.bar_pointed = on;
        changed
    });
    if changed {
        view.redraw(false);
    }
}

/// A press at `at`: on the slider it is taken where it was pressed; in its track the slider is
/// brought there first. Whether the press was the scrollbar's.
pub(super) fn press(view: &View, at: (f64, f64)) -> bool {
    let Some((top, length, ..)) = slider(view).filter(|_| over(view, at)) else {
        return false;
    };
    let grip = acts::grip(at.1, (top, length));
    if let Some(mut state) = view.state() {
        state.bar_held = Some(grip);
        state.bar_pointed = true;
    }
    drag(view, at)
}

/// The slider dragged to `at`: the view scrolled to the rows it stands for. Whether it is held.
pub(super) fn drag(view: &View, (_, y): (f64, f64)) -> bool {
    let held = view.state().and_then(|state| state.bar_held);
    let (Some(grip), Some((_, length, height, bar))) = (held, slider(view)) else {
        return false;
    };
    let row = acts::bar_row(y, grip, (length, height), bar);
    if let Some(surface) = view.surface() {
        surface.scroll_to(ScrollTo::Row(row));
    }
    view.redraw(false);
    true
}

/// The press let go: whether it was the scrollbar's.
pub(super) fn release(view: &View) -> bool {
    let held = view.state().and_then(|mut state| state.bar_held.take());
    if held.is_some() {
        view.redraw(false);
    }
    held.is_some()
}
