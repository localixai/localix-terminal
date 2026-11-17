//! The pointer in the view, told to the terminal: to the program when it takes the mouse, else a
//! selection by the core's gestures, scrolled on at the display's pace while it is dragged past
//! an edge (a display link that runs only then); the wheel and the trackpad in whole rows; the
//! menu a right click opens; and what calls the view from outside, let go of as it closes.

use std::sync::Arc;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send, sel};
use objc2_app_kit::NSEvent;
use objc2_core_foundation::CGRect;
use objc2_foundation::NSString;

use super::acts::{self, Picked};
use super::events::{self, surface};
use super::view::TermView;
use super::{keymap, ns};
use crate::surface::{Button, Edge, Mouse, MouseAction, Routed};

/// What the pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pointer {
    Down,
    Dragged,
    Up,
    Moved,
}

/// The pointer's place in the view, in pixels from its top left.
fn at(view: &TermView, event: &NSEvent) -> (f32, f32) {
    let point = ns::at(view, event);
    let scale = ns::scale(view);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a place in a view, in pixels"
    )]
    let point = ((point.x * scale) as f32, (point.y * scale) as f32);
    point
}

/// A pointer event as the terminal reads it.
fn read(view: &TermView, event: &NSEvent, pointer: Pointer) -> Mouse {
    let (x, y) = at(view, event);
    let button = match (pointer, ns::button(event)) {
        (Pointer::Moved, _) => None,
        (_, 0) => Some(Button::Left),
        (_, 1) => Some(Button::Right),
        (_, 2) => Some(Button::Middle),
        _ => None,
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an event's time since the system started, in nanoseconds"
    )]
    let time_ns = (ns::time(event).max(0.0) * 1e9) as u64;
    Mouse {
        x,
        y,
        button,
        action: match pointer {
            Pointer::Down => MouseAction::Press,
            Pointer::Up => MouseAction::Release,
            Pointer::Dragged | Pointer::Moved => MouseAction::Motion,
        },
        mods: keymap::mods(ns::flags(event)),
        time_ns,
    }
}

pub(super) fn mouse(view: &TermView, event: &NSEvent, pointer: Pointer) {
    let Some(surface) = surface(view) else {
        return;
    };
    if pointer == Pointer::Down {
        view.focus();
    }
    // A Command-click on a link opens it; the scrollbar takes what is pressed on it.
    let point = ns::at(view, event);
    let at = (point.x, point.y);
    let flags = ns::flags(event);
    let theirs = match pointer {
        Pointer::Down => {
            super::links::open(view, at, keymap::mods(flags)) || super::bar::press(view, at)
        }
        Pointer::Dragged => super::bar::drag(view, at),
        Pointer::Up => super::bar::release(view),
        Pointer::Moved => {
            super::links::hover(view, at, flags);
            super::bar::moved(view, at);
            false
        }
    };
    if theirs {
        return;
    }
    let taken = view.state().is_some_and(|state| state.frame.mouse_taken);
    // A pointer that only moves is the program's alone: no selection follows it.
    if pointer == Pointer::Moved && !taken {
        return;
    }
    let Some(geometry) = view.geometry() else {
        return;
    };
    let mouse = read(view, event, pointer);
    let routed = surface.mouse(&mouse, &geometry);
    match (routed, pointer) {
        (Routed::Selection { scroll: Some(edge) }, Pointer::Dragged) => drag(view, edge, mouse),
        _ => undrag(view),
    }
    view.redraw(false);
}

/// A display link of `view`'s screen calling its `dragFrame:` each frame, in the main run
/// loop's common modes; none before macOS 14, where the selection scrolls as the pointer moves.
fn display_link(view: &TermView) -> Option<Retained<AnyObject>> {
    let selector = sel!(displayLinkWithTarget:selector:);
    // SAFETY: an object answers whether it responds to a selector.
    let can: bool = unsafe { msg_send![view, respondsToSelector: selector] };
    if !can {
        return None;
    }
    // SAFETY: a view makes a display link calling its target's selector each frame.
    let link: Retained<AnyObject> =
        unsafe { msg_send![view, displayLinkWithTarget: view, selector: sel!(dragFrame:)] };
    // SAFETY: the main run loop, always there.
    let run_loop: Retained<AnyObject> = unsafe { msg_send![class!(NSRunLoop), mainRunLoop] };
    let modes = NSString::from_str("kCFRunLoopCommonModes");
    // SAFETY: the link runs in that loop's modes until it is invalidated (`undrag`).
    let _: () = unsafe { msg_send![&*link, addToRunLoop: &*run_loop, forMode: &*modes] };
    Some(link)
}

/// A selection dragged past `edge`: the view scrolls on a row a display frame while it stays.
fn drag(view: &TermView, edge: Edge, mouse: Mouse) {
    if let Some(mut state) = view.state()
        && let Some(dragging) = state.dragging.as_mut()
    {
        dragging.0 = edge;
        dragging.2 = mouse;
        return;
    }
    let Some(link) = display_link(view) else {
        return;
    };
    // A link the view cannot keep (its state held by a call the system made back into it) is
    // stopped at once: the run loop would keep it, and it the view, with nothing to stop it.
    match view.state() {
        Some(mut state) => state.dragging = Some((edge, link, mouse)),
        None => stop(&link),
    }
}

fn undrag(view: &TermView) {
    let link = view.state().and_then(|mut state| state.dragging.take());
    if let Some((_, link, _)) = link {
        stop(&link);
    }
}

/// A display link stopped for good: out of its run loop, its hold on the view let go of.
fn stop(link: &AnyObject) {
    // SAFETY: a display link answers `invalidate`, which takes nothing.
    let _: () = unsafe { msg_send![link, invalidate] };
}

/// A display frame while a selection is dragged past an edge: a row further, the selection
/// taken to where the pointer is.
pub(super) fn drag_frame(view: &TermView) {
    let dragging = view.state().and_then(|state| {
        let (edge, _, mouse) = state.dragging.as_ref()?;
        Some((*edge, *mouse, Arc::clone(&state.surface)))
    });
    let (Some((edge, mouse, surface)), Some(geometry)) = (dragging, view.geometry()) else {
        return undrag(view);
    };
    let rows = if edge == Edge::Top { 1 } else { -1 };
    surface.wheel(
        rows,
        &Mouse {
            button: None,
            ..mouse
        },
        &geometry,
    );
    if surface.mouse(&mouse, &geometry) == (Routed::Selection { scroll: None }) {
        undrag(view);
    }
    view.redraw(false);
}

/// The wheel or the trackpad: whole rows, a trackpad's fine turns gathered until they make one.
pub(super) fn wheel(view: &TermView, event: &NSEvent) {
    let Some(surface) = surface(view) else {
        return;
    };
    let (dy, precise) = ns::turn(event);
    let rows = {
        let Some(mut state) = view.state() else {
            return;
        };
        let tall = state
            .painter
            .as_ref()
            .map_or(16.0, |painter| painter.cell().1);
        acts::wheel_rows(&mut state.wheel, dy, precise, tall)
    };
    if rows == 0 {
        return;
    }
    let Some(geometry) = view.geometry() else {
        return;
    };
    surface.wheel(rows, &read(view, event, Pointer::Moved), &geometry);
    view.redraw(false);
}

/// The terminal's menu where the person right-clicked: Copy while something is selected, Paste,
/// Select All, and Clear on the main screen (a full-screen program draws all of its own).
pub(super) fn right_down(view: &TermView, event: &NSEvent) {
    let Some(surface) = surface(view) else {
        return;
    };
    view.focus();
    let main = view.state().is_some_and(|state| !state.frame.alternate);
    let (entries, picks) = acts::menu(surface.copy().is_some(), main);
    let point = ns::at(view, event);
    let picked = crate::browser::menu::show_in(view, (point.x, point.y), &entries);
    match picked.and_then(|place| picks.get(place)) {
        Some(Picked::Copy) => events::copy(view),
        Some(Picked::Paste) => events::paste(view),
        Some(Picked::SelectAll) => events::select_all(view),
        Some(Picked::Clear) => {
            surface.clear();
            view.redraw(false);
        }
        None => {}
    }
}

/// The view hears the pointer come, move over it and go, its shape asked for, and its window's
/// keys come and go.
pub(super) fn track(view: &TermView) {
    // NSTrackingMouseEnteredAndExited, NSTrackingMouseMoved, NSTrackingCursorUpdate,
    // NSTrackingActiveInKeyWindow, NSTrackingInVisibleRect.
    let options: usize = 0x01 | 0x02 | 0x04 | 0x20 | 0x200;
    let zero = CGRect::default();
    let none = std::ptr::null::<AnyObject>();
    // SAFETY: NSTrackingArea's allocation, initialized below.
    let area: Allocated<AnyObject> = unsafe { msg_send![class!(NSTrackingArea), alloc] };
    // SAFETY: a tracking area for the view (its rect follows the visible one).
    let area: Retained<AnyObject> = unsafe {
        msg_send![area, initWithRect: zero, options: options, owner: view, userInfo: none]
    };
    // SAFETY: the view keeps its tracking area.
    let _: () = unsafe { msg_send![view, addTrackingArea: &*area] };
    // SAFETY: the process's notification centre, always there.
    let center: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNotificationCenter), defaultCenter] };
    for name in [
        "NSWindowDidBecomeKeyNotification",
        "NSWindowDidResignKeyNotification",
    ] {
        let name = NSString::from_str(name);
        let selector = sel!(windowKeyChanged:);
        // SAFETY: the centre calls the view's selector for the windows' key changes; it holds
        // the view weakly and forgets it as the view is freed.
        let _: () = unsafe {
            msg_send![&*center, addObserver: view, selector: selector, name: &*name, object: none]
        };
    }
}

/// The view let go of what calls it from outside: a selection's display link stopped (it holds
/// the view), its tracking areas taken off, and the window's key notices no longer sent to it.
pub(super) fn untrack(view: &TermView) {
    undrag(view);
    // SAFETY: an NSView answers its tracking areas, an NSArray, copied so that it does not
    // change as they are taken off.
    let areas: Retained<AnyObject> = unsafe { msg_send![view, trackingAreas] };
    // SAFETY: as above.
    let areas: Retained<AnyObject> = unsafe { msg_send![&*areas, copy] };
    // SAFETY: an NSArray answers how many it holds, an NSUInteger.
    let count: usize = unsafe { msg_send![&*areas, count] };
    for at in 0..count {
        // SAFETY: an NSArray answers the object at an index below its count.
        let area: Retained<AnyObject> = unsafe { msg_send![&*areas, objectAtIndex: at] };
        // SAFETY: the view takes off a tracking area of its own.
        let _: () = unsafe { msg_send![view, removeTrackingArea: &*area] };
    }
    // SAFETY: the process's notification centre, always there.
    let center: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNotificationCenter), defaultCenter] };
    // SAFETY: the centre stops calling the view for every notice it was added for.
    let _: () = unsafe { msg_send![&*center, removeObserver: view] };
}
