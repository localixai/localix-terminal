//! A choice put to the person as the system's own menu (macOS), where they clicked: what the
//! strip's buttons offer ("+": a page or a terminal). It looks and behaves as every menu of the
//! system does, covers nothing but itself, and answers the item picked, or none.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use wry::{WebView, WebViewExtMacOS};

use super::capture::CGPoint;

/// The item the person picked in the menu shown now: its tag, -1 for none.
static PICKED: AtomicIsize = AtomicIsize::new(-1);

extern "C" fn pick(_this: &AnyObject, _sel: Sel, sender: *mut AnyObject) {
    // SAFETY: the sender is the NSMenuItem picked, alive while its action runs.
    let Some(item) = (unsafe { sender.as_ref() }) else {
        return;
    };
    // SAFETY: an NSMenuItem answers its tag, an NSInteger.
    let tag: isize = unsafe { msg_send![item, tag] };
    PICKED.store(tag, Ordering::SeqCst);
}

/// What the items call: one object for the process.
fn picker() -> Option<&'static AnyObject> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    let at = (*TARGET.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxMenuPick", AnyClass::get(c"NSObject")?)?;
        // SAFETY: an action's signature: the receiver, the selector, the sender.
        unsafe { class.add_method(sel!(pick:), pick as extern "C" fn(_, _, _)) };
        let class = class.register();
        // SAFETY: `new` on an NSObject subclass answers a retained instance.
        let object: Retained<AnyObject> = unsafe { msg_send![class, new] };
        // Kept for the process: a menu item holds its target weakly.
        Some(Retained::into_raw(object) as usize)
    }))?;
    // SAFETY: the object was made above and never released.
    unsafe { (at as *const AnyObject).as_ref() }
}

/// `words` as an NSString; none when they hold a NUL.
fn text(words: &str) -> Option<Retained<AnyObject>> {
    let words = std::ffi::CString::new(words).ok()?;
    // SAFETY: a NUL-terminated UTF-8 string, alive through the call; it answers a new string.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: words.as_ptr()] }
}

/// Show `items` as a menu at `at` (the strip's points, from its top left) and wait for the
/// person: the index of the item picked, or none when they dismissed it.
pub(super) fn choose(
    strip: &WebView,
    at: (f64, f64),
    items: &[&str],
    marked: Option<usize>,
) -> Option<usize> {
    let target = picker()?;
    // SAFETY: `new` on NSMenu answers a retained, empty menu.
    let menu: Retained<AnyObject> = unsafe { msg_send![class!(NSMenu), new] };
    // SAFETY: a menu takes a BOOL: its items are enabled as they are set, not by a responder.
    let _: () = unsafe { msg_send![&*menu, setAutoenablesItems: Bool::NO] };
    let none = text("")?;
    for (tag, title) in items.iter().enumerate() {
        let Some(title) = text(title) else {
            continue;
        };
        // SAFETY: a menu adds an item of a title, an action and a key, and answers it.
        let item: *mut AnyObject = unsafe {
            msg_send![&*menu, addItemWithTitle: &*title, action: sel!(pick:), keyEquivalent: &*none]
        };
        // SAFETY: the menu keeps the item it answered.
        let Some(item) = (unsafe { item.as_ref() }) else {
            continue;
        };
        // SAFETY: an NSMenuItem takes a target.
        let _: () = unsafe { msg_send![item, setTarget: target] };
        // SAFETY: an NSMenuItem takes an NSInteger tag.
        let _: () = unsafe { msg_send![item, setTag: isize::try_from(tag).unwrap_or(-1)] };
        if marked == Some(tag) {
            /// NSControlStateValueOn: the item's check mark.
            const ON: isize = 1;
            // SAFETY: an NSMenuItem takes an NSControlStateValue (an NSInteger).
            let _: () = unsafe { msg_send![item, setState: ON] };
        }
    }
    PICKED.store(-1, Ordering::SeqCst);
    let view = strip.webview();
    let point = CGPoint { x: at.0, y: at.1 };
    // The menu runs until the person picks or dismisses it; the window keeps drawing meanwhile.
    // A WKWebView's coordinates run downwards, as the page's do.
    // SAFETY: a menu pops up over a view, at a point of it, with no item placed under it.
    let _: Bool = unsafe {
        msg_send![&*menu, popUpMenuPositioningItem: std::ptr::null::<AnyObject>(), atLocation: point, inView: &*view]
    };
    usize::try_from(PICKED.load(Ordering::SeqCst)).ok()
}
