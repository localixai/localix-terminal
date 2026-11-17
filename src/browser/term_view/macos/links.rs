//! The links in the view: while Command is held, the link under the pointer (one its program gave,
//! or an http or https address in its line) is underlined and the pointer is a hand; a
//! Command-click opens it, in a tab of the window. Without Command a click selects or goes to the
//! program as any other; and the pointer's shape over the view.

use std::sync::Arc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use super::view::TermView;
use crate::surface::{Link, Mods, Surface};

/// The modifier flags' bit for Command.
const COMMAND: usize = 1 << 20;

/// The cell under `at` (the view's points from its top left), as the grid is drawn now.
fn cell(view: &TermView, at: (f64, f64)) -> Option<(u16, u16)> {
    let state = view.state()?;
    super::acts::cell_at(&state.frame, state.painter.as_ref()?.cell(), at)
}

/// The link under `at` while `command` is held, as the terminal says it.
fn under(view: &TermView, at: (f64, f64), command: bool) -> Option<Link> {
    let surface: Arc<dyn Surface> = super::events::surface(view)?;
    let (column, y) = cell(view, at).filter(|_| command)?;
    surface.link_at(column, y)
}

/// The pointer is at `at` with `flags` held: the link under it underlined while Command is,
/// and the pointer's shape for it.
pub(super) fn hover(view: &TermView, at: (f64, f64), flags: usize) {
    let link = under(view, at, flags & COMMAND != 0);
    let changed = view.state().is_some_and(|mut state| {
        state.pointer = Some(at);
        let changed = state.link != link;
        if changed {
            state.frame.link = link
                .as_ref()
                .map(|link| link.spans.clone())
                .unwrap_or_default();
            state.link = link;
        }
        changed
    });
    if changed {
        shape(view);
        view.redraw(true);
    }
}

/// Command pressed or let go where the pointer last was: the link under it lit or not.
pub(super) fn flags(view: &TermView, flags: usize) {
    if let Some(at) = view.state().and_then(|state| state.pointer) {
        hover(view, at, flags);
    }
}

/// A press at `at` with `mods` held: the link under it opened when Command is; whether it was.
pub(super) fn open(view: &TermView, at: (f64, f64), mods: Mods) -> bool {
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
pub(super) fn shape(view: &TermView) {
    let (link, taken) = view.state().map_or((false, false), |state| {
        (state.link.is_some(), state.frame.mouse_taken)
    });
    let cursor: Retained<AnyObject> = match (link, taken) {
        // SAFETY: NSCursor answers the system's pointing hand.
        (true, _) => unsafe { msg_send![class!(NSCursor), pointingHandCursor] },
        // SAFETY: and its arrow.
        (false, true) => unsafe { msg_send![class!(NSCursor), arrowCursor] },
        // SAFETY: and its I-beam.
        (false, false) => unsafe { msg_send![class!(NSCursor), IBeamCursor] },
    };
    // SAFETY: a cursor is made the current one.
    let _: () = unsafe { msg_send![&*cursor, set] };
}
