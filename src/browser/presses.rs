//! The person's presses in the app's windows, wherever they land: a page or a terminal takes
//! its own clicks, so the window hears of them here (a local event monitor) and knows which half
//! of a split window the person works in. A press in the window (a panel that never activates
//! the app by itself, nspanel.rs) makes llx-desk the app in use, as a click in any app's window
//! does: its menus in the menu bar, its window key. What an agent does never activates it.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use tao::event_loop::EventLoopProxy;

use super::capture::{CGPoint, CGRect};
use super::host::Cmd;

/// NSEventMaskLeftMouseDown.
const LEFT_DOWN: u64 = 1 << 1;

/// Tell `proxy` of each left press in one of the app's windows, for the rest of the process:
/// the window, and where, in points from its top left. An unseen window (a test's) is never
/// watched.
pub(super) fn watch(proxy: &EventLoopProxy<Cmd>) {
    if super::platform::unseen() {
        return;
    }
    let proxy = proxy.clone();
    let block = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
        take_over(event);
        if let Some((window, x, y)) = pressed(event) {
            let _ = proxy.send_event(Cmd::Pressed(window, x, y));
        }
        event
    });
    // SAFETY: a mask and a block of `NSEvent * (^)(NSEvent *)`, which NSEvent copies; it answers
    // the monitor (autoreleased, retained here). The block hands each event on, unchanged.
    let monitor: Option<Retained<AnyObject>> = unsafe {
        msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: LEFT_DOWN, handler: &*block]
    };
    // Kept for the process: removed, the halves would no longer follow the person's presses.
    if let Some(monitor) = monitor {
        let _ = Retained::into_raw(monitor);
    }
}

/// NSWindowLevel of the window.
const NORMAL: isize = 0;

/// Press `event` in the window, llx-desk not the active app: the app made the active one, and
/// the window its key window, the person's keys going to it.
fn take_over(event: *mut AnyObject) {
    // SAFETY: the monitor hands an NSEvent, or nil.
    let Some(event) = (unsafe { event.as_ref() }) else {
        return;
    };
    // SAFETY: an NSEvent answers its window, nil for none.
    let window: *mut AnyObject = unsafe { msg_send![event, window] };
    // SAFETY: the window answered, or nil.
    let Some(window) = (unsafe { window.as_ref() }) else {
        return;
    };
    // SAFETY: an NSWindow answers its NSWindowLevel (an NSInteger).
    let level: isize = unsafe { msg_send![window, level] };
    if level != NORMAL {
        return;
    }
    // Only the app's own windows: the system's own (the green button's tiling menu is one, in
    // this process) must not be made key by a press, or they close before they act.
    let Some(ours) = objc2::runtime::AnyClass::get(c"TaoWindow") else {
        return;
    };
    // SAFETY: `isKindOfClass:` takes a class and answers a BOOL.
    let is_ours: objc2::runtime::Bool = unsafe { msg_send![window, isKindOfClass: ours] };
    if !is_ours.as_bool() {
        return;
    }
    super::appkit::activate_and_key(window);
}

/// The window of press `event`, and where in it, from its content's top left.
fn pressed(event: *mut AnyObject) -> Option<(usize, f64, f64)> {
    // SAFETY: the monitor hands an NSEvent, or nil.
    let event = unsafe { event.as_ref() }?;
    // SAFETY: an NSEvent answers its window, nil for none.
    let window: *mut AnyObject = unsafe { msg_send![event, window] };
    // SAFETY: the window answered, or nil.
    let shown = unsafe { window.as_ref() }?;
    // SAFETY: an NSEvent of a mouse answers where it was in its window, an NSPoint.
    let at: CGPoint = unsafe { msg_send![event, locationInWindow] };
    // SAFETY: an NSWindow answers its content view.
    let view: *mut AnyObject = unsafe { msg_send![shown, contentView] };
    // SAFETY: the view answered, or nil.
    let view = unsafe { view.as_ref() }?;
    // SAFETY: an NSView answers its bounds, an NSRect.
    let bounds: CGRect = unsafe { msg_send![view, bounds] };
    Some((window.addr(), at.x, bounds.size.height - at.y))
}
