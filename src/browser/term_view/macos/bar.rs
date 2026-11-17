//! The scrollbar over the view's right edge, as the terminal page had it: its slider shows only
//! while there is history to scroll and the pointer is over the view, and fades out as it
//! leaves (gone at once under Reduce Motion); pointed at it is brighter; dragged, the view
//! follows it, and a press in its track brings the slider there.

use super::overlay;
use super::view::TermView;
use crate::surface::ScrollTo;

/// How long the slider takes to come and to go (the page's: its entrance, and its fade out).
const COMING: f64 = 0.1;
const GOING: f64 = 0.8;

/// The pointer came over the view (`inside`) or left it: the slider shows, or fades.
pub(super) fn pointer_in(view: &TermView, inside: bool) {
    let Some(mut state) = view.state() else {
        return;
    };
    state.bar_shown = inside;
    if !inside {
        state.bar_pointed = false;
    }
    let fade = match (inside, crate::browser::appkit::reduced_motion()) {
        (_, true) => 0.0,
        (true, false) => COMING,
        (false, false) => GOING,
    };
    state
        .layers
        .bar_shown(inside || state.bar_held.is_some(), fade);
}

/// The slider's top and length in a view `height` points tall, and the rows it stands for.
fn slider(view: &TermView) -> Option<(f64, f64, f64, crate::surface::Scrollbar)> {
    let bounds = super::ns::bounds(view);
    let state = view.state()?;
    let (top, length) = overlay::scrollbar(&state.frame, bounds.size.height)?;
    Some((top, length, bounds.size.height, state.frame.scroll))
}

/// Whether `at` (the view's points) is over the scrollbar's column while it shows.
fn over(view: &TermView, (x, _): (f64, f64)) -> bool {
    let width = super::ns::bounds(view).size.width;
    let shown = view.state().is_some_and(|state| state.bar_shown);
    shown && super::acts::on_bar(x, width) && slider(view).is_some()
}

/// The pointer moved to `at`: the slider brighter while it is on it.
pub(super) fn moved(view: &TermView, at: (f64, f64)) {
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
pub(super) fn press(view: &TermView, at: (f64, f64)) -> bool {
    let Some((top, length, ..)) = slider(view).filter(|_| over(view, at)) else {
        return false;
    };
    let grip = super::acts::grip(at.1, (top, length));
    if let Some(mut state) = view.state() {
        state.bar_held = Some(grip);
        state.bar_pointed = true;
    }
    drag(view, at)
}

/// The slider dragged to `at`: the view scrolled to the rows it stands for. Whether it is held.
pub(super) fn drag(view: &TermView, (_, y): (f64, f64)) -> bool {
    let held = view.state().and_then(|state| state.bar_held);
    let (Some(grip), Some((_, length, height, bar))) = (held, slider(view)) else {
        return false;
    };
    let row = super::acts::bar_row(y, grip, (length, height), bar);
    if let Some(surface) = super::events::surface(view) {
        surface.scroll_to(ScrollTo::Row(row));
    }
    view.redraw(false);
    true
}

/// The press let go: whether it was the scrollbar's.
pub(super) fn release(view: &TermView) -> bool {
    let held = view.state().and_then(|mut state| state.bar_held.take());
    if held.is_some() {
        let shown = view.state().is_some_and(|state| state.bar_shown);
        if !shown {
            pointer_in(view, false);
        }
        view.redraw(false);
    }
    held.is_some()
}
