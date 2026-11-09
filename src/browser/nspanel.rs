//! The deck's window in AppKit's own messages (macOS): an ordinary window, native in tiling and
//! full screen, revealed and concealed without ever activating the app for an agent, and its
//! minimize button put to the window's own way out of sight (presence.rs). Nothing here
//! activates the app but the person's click.

use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tao::event_loop::EventLoopProxy;
use tao::window::Window;

use super::appkit::masks::{FLOATING_LEVEL, FULL_SCREEN_PRIMARY, NORMAL_LEVEL};
use super::appkit::native;
use super::capture::CGRect;
use super::host::Cmd;
use super::placement::Area;
use super::platform::unseen;
use super::presence::Event;

/// Where the main window's own minimize button goes: the loop, told to put the window away.
static COLLAPSE: OnceLock<Mutex<Option<EventLoopProxy<Cmd>>>> = OnceLock::new();

/// The minimize button (and ⌘M) puts the main window out of sight, still drawing: minimized into
/// the Dock, WebKit would stop its pages, and an agent's work in them with it.
extern "C" fn collapse(_this: &AnyObject, _sel: Sel, _sender: *mut AnyObject) {
    let proxy = COLLAPSE.get().and_then(|held| crate::lock(held).clone());
    if let Some(proxy) = proxy {
        let _ = proxy.send_event(Cmd::Presence(Event::Collapse));
    }
}

/// What the minimize button calls instead of minimizing: one object for the process.
fn collapser() -> Option<&'static AnyObject> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    let at = (*TARGET.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxCollapse", AnyClass::get(c"NSObject")?)?;
        // SAFETY: an action's signature: the receiver, the selector, the sender.
        unsafe { class.add_method(sel!(collapse:), collapse as extern "C" fn(_, _, _)) };
        let class = class.register();
        // SAFETY: `new` on an NSObject subclass answers a retained instance.
        let object: Retained<AnyObject> = unsafe { msg_send![class, new] };
        // Kept for the process: a button holds its target weakly.
        Some(Retained::into_raw(object) as usize)
    }))?;
    // SAFETY: the object was made above and never released.
    unsafe { (at as *const AnyObject).as_ref() }
}

/// The main `window`'s minimize button puts it away, told to `proxy`. Its green
/// button keeps the system's own: full screen, and its tiling menu.
pub(super) fn collapse_to(window: &Window, proxy: &EventLoopProxy<Cmd>) {
    *crate::lock(COLLAPSE.get_or_init(|| Mutex::new(None))) = Some(proxy.clone());
    hook_collapse(window);
}

/// The minimize button of `window` puts it away again (the system makes its buttons anew
/// when full screen ends).
pub(super) fn hook_collapse(window: &Window) {
    /// NSWindowButton: the minimize button.
    const MINIATURIZE: usize = 1;
    let Some(target) = collapser() else {
        return;
    };
    // SAFETY: a window answers its standard button of a kind, or nil.
    let button: *mut AnyObject =
        unsafe { msg_send![native(window), standardWindowButton: MINIATURIZE] };
    // SAFETY: checked for nil; the window keeps its buttons alive.
    let Some(button) = (unsafe { button.as_ref() }) else {
        return;
    };
    // SAFETY: an NSControl takes a target and an action.
    let _: () = unsafe { msg_send![button, setTarget: target] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![button, setAction: sel!(collapse:)] };
}

/// The deck's collection behaviour: an ordinary window among the person's, native in tiling and
/// full screen. It may go full screen and be tiled: macOS offers its window tiling (the green
/// button's menu, dragging to an edge or a corner, Option-drag) only to windows that may.
const DECK_SPACES: usize = FULL_SCREEN_PRIMARY;

/// Make `window` the deck's: the app's main window, which a click activates and the green button
/// takes to full screen. Shown by an agent, it still comes to the front without taking the keys
/// (`reveal`).
pub(super) fn adopt(window: &Window) {
    let object = native(window);
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setHidesOnDeactivate: Bool::NO] };
    // SAFETY: an NSWindow takes an NSWindowCollectionBehavior (an NSUInteger).
    let _: () = unsafe { msg_send![object, setCollectionBehavior: DECK_SPACES] };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: NORMAL_LEVEL] };
}

/// On screen, as it looks: opaque to the eye and the mouse. Under `DESK_WINDOWS=unseen` it stays
/// invisible and click-through, as every window of a test run does.
pub(super) fn reveal(window: &Window, rendering: Rendering) {
    if unseen() {
        return conceal(window, rendering);
    }
    let object = native(window);
    // SAFETY: an NSWindow takes a CGFloat.
    let _: () = unsafe { msg_send![object, setAlphaValue: 1.0f64] };
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setIgnoresMouseEvents: Bool::NO] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![object, setHasShadow: Bool::YES] };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: NORMAL_LEVEL] };
    // SAFETY: an NSWindow answers a BOOL.
    let here: Bool = unsafe { msg_send![object, isOnActiveSpace] };
    if !here.as_bool() {
        // Put away, it stayed on the Space it was on: taken off the screen and put back in the
        // same turn, it comes back on the Space the person is on now. On that Space already, it
        // is only brought forward: nothing flickers.
        // SAFETY: `orderOut:` takes a sender, which may be nil, and returns nothing.
        let _: () = unsafe { msg_send![object, orderOut: std::ptr::null::<AnyObject>()] };
    }
    front(object);
}

/// How an invisible window keeps its page drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rendering {
    /// WebKit's occlusion switch is off: the window may sit behind every other one.
    Switched,
    /// It is not: the window floats on screen, clear, so that nothing covers it.
    Floating,
}

/// Out of sight and out of the way, and still drawing, at the size it has: clear, passing every
/// click through, behind the other windows (what a headless window is, and WebKit keeps
/// rendering in it, so a page never reflows or stops).
pub(super) fn conceal(window: &Window, rendering: Rendering) {
    conceal_object(native(window), rendering);
}

/// [`conceal`] for a window found through its WebView.
pub(super) fn conceal_object(object: &AnyObject, rendering: Rendering) {
    // SAFETY: an NSWindow takes a CGFloat.
    let _: () = unsafe { msg_send![object, setAlphaValue: 0.0f64] };
    // SAFETY: an NSWindow takes a BOOL.
    let _: () = unsafe { msg_send![object, setIgnoresMouseEvents: Bool::YES] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![object, setHasShadow: Bool::NO] };
    let level: isize = if rendering == Rendering::Switched {
        NORMAL_LEVEL
    } else {
        FLOATING_LEVEL
    };
    // SAFETY: an NSWindow takes an NSWindowLevel (an NSInteger).
    let _: () = unsafe { msg_send![object, setLevel: level] };
    if rendering == Rendering::Switched {
        // SAFETY: `orderBack:` takes a sender, which may be nil, and returns nothing.
        let _: () = unsafe { msg_send![object, orderBack: std::ptr::null::<AnyObject>()] };
    } else {
        front(object);
    }
}

/// Tell `proxy` each time the person changes what Accessibility, Display offers (reduce
/// transparency, increase contrast, reduce motion), for the rest of the process: the windows are
/// dressed again, as they are when a setting of the app's changes.
pub(super) fn watch_display(proxy: &EventLoopProxy<Cmd>) {
    watch_workspace(
        "NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification",
        proxy,
        || Cmd::Restyle,
    );
}

/// `tell`'s command sent to `proxy` each time the shared workspace posts notification `name`.
fn watch_workspace(name: &str, proxy: &EventLoopProxy<Cmd>, tell: fn() -> Cmd) {
    use block2::RcBlock;
    if unseen() {
        return;
    }
    let proxy = proxy.clone();
    let block = RcBlock::new(move |_note: *mut AnyObject| {
        let _ = proxy.send_event(tell());
    });
    // SAFETY: `sharedWorkspace` is NSWorkspace's class method, returning the shared instance.
    let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    // SAFETY: an NSWorkspace answers its notification center.
    let center: Retained<AnyObject> = unsafe { msg_send![&*workspace, notificationCenter] };
    let name = super::appkit::text(name);
    // SAFETY: `mainQueue` is NSOperationQueue's class method: the block runs on the main thread.
    let queue: Retained<AnyObject> = unsafe { msg_send![class!(NSOperationQueue), mainQueue] };
    // SAFETY: a name, a nil object, a queue and a block of `void (^)(NSNotification *)`, which
    // the center copies; it answers the observer (autoreleased, retained here).
    let observer: Option<Retained<AnyObject>> = unsafe {
        msg_send![&*center, addObserverForName: &*name, object: std::ptr::null::<AnyObject>(), queue: &*queue, usingBlock: &*block]
    };
    // Kept for the process: removed, the person's changes would go unwatched.
    if let Some(observer) = observer {
        let _ = Retained::into_raw(observer);
    }
}

/// llx-desk the active app and `window` its key window, for the person who just opened it:
/// their next click and key go to it, as to any app they bring forward. Only their click calls it.
pub(super) fn focus(window: &Window) {
    if unseen() {
        return;
    }
    super::appkit::activate_and_key(native(window));
}

/// llx-desk no longer the active app, for the person who just put its window away: the app they
/// were in before takes their keys again. Only their click calls it.
pub(super) fn give_up_keys() {
    if unseen() {
        return;
    }
    super::appkit::deactivate();
}

/// Whether `window` is the key window: the one the keys go to.
pub(super) fn is_key(window: &Window) -> bool {
    // SAFETY: an NSWindow answers whether it is the key window, a BOOL.
    let key: Bool = unsafe { msg_send![native(window), isKeyWindow] };
    key.as_bool()
}

fn front(object: &AnyObject) {
    // SAFETY: `orderFrontRegardless` takes no argument and returns nothing; unlike
    // `makeKeyAndOrderFront:` it neither makes the window key nor activates the app.
    let _: () = unsafe { msg_send![object, orderFrontRegardless] };
}

/// The primary screen's room for windows, below its menu bar and above its Dock, in points from
/// the top left.
pub(super) fn work_area() -> Option<Area> {
    // SAFETY: `screens` is NSScreen's class method, answering an array.
    let screens: Retained<AnyObject> = unsafe { msg_send![class!(NSScreen), screens] };
    // SAFETY: an NSArray answers `count` with an NSUInteger.
    let count: usize = unsafe { msg_send![&*screens, count] };
    if count == 0 {
        return None;
    }
    // SAFETY: the array has an element at 0 (checked just above).
    let primary: Retained<AnyObject> = unsafe { msg_send![&*screens, objectAtIndex: 0usize] };
    // SAFETY: an NSScreen answers `frame` with an NSRect, which `CGRect` lays out.
    let frame: CGRect = unsafe { msg_send![&*primary, frame] };
    // SAFETY: as above, for `visibleFrame`.
    let visible: CGRect = unsafe { msg_send![&*primary, visibleFrame] };
    Some(Area {
        x: visible.origin.x,
        y: frame.size.height - visible.origin.y - visible.size.height,
        width: visible.size.width,
        height: visible.size.height,
    })
}

#[cfg(test)]
mod tests {
    use super::super::appkit::masks::{CAN_JOIN_ALL_SPACES, FULL_SCREEN_AUXILIARY};
    use super::*;

    #[test]
    fn deck_goes_full_screen_and_tiles_as_an_ordinary_window() {
        let behavior = DECK_SPACES;
        assert_ne!(behavior & FULL_SCREEN_PRIMARY, 0);
        assert_eq!(behavior & FULL_SCREEN_AUXILIARY, 0);
        assert_eq!(behavior & CAN_JOIN_ALL_SPACES, 0);
    }
}
