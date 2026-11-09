//! AppKit's few common messages, once (macOS): a string, the app, the person's question
//! (`sheet`).

use std::ffi::CStr;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use tao::platform::macos::WindowExtMacOS;

mod sheet;
pub(super) use sheet::{confirm, pick_folder};

/// `text` as an NSString; none when it holds a NUL.
pub(super) fn string(text: &str) -> Option<Retained<AnyObject>> {
    let text = std::ffi::CString::new(text).ok()?;
    literal(&text)
}

/// `text` as an NSString, any NUL in it left out.
pub(super) fn text(text: &str) -> Retained<AnyObject> {
    string(&text.replace('\0', "")).unwrap_or_else(|| {
        // SAFETY: `new` on NSString answers an empty string, retained.
        unsafe { msg_send![class!(NSString), new] }
    })
}

/// A C string as an NSString (a constant's name, a key).
pub(super) fn literal(text: &CStr) -> Option<Retained<AnyObject>> {
    // SAFETY: a NUL-terminated UTF-8 string, alive through the call; it answers a new string.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
}

/// `window`'s NSWindow, for as long as the borrow lasts.
pub(super) fn native(window: &tao::window::Window) -> &AnyObject {
    // SAFETY: tao's NSWindow is alive for as long as its `Window` is, which is borrowed here.
    unsafe { &*window.ns_window().cast::<AnyObject>() }
}

/// The app (NSApplication).
pub(super) fn app() -> Retained<AnyObject> {
    // SAFETY: NSApplication's class method answers the app.
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

/// The app made the one the person uses: only ever for what they did themselves (the Dock's
/// icon, quitting), never for what an agent does.
pub(super) fn activate() {
    // SAFETY: the app takes a BOOL: whether to take over from the app active now.
    let _: () = unsafe { msg_send![&*app(), activateIgnoringOtherApps: Bool::YES] };
}

/// The app no longer the one the person uses: the system activates the one they were in. Only
/// ever for what they did themselves (putting the window away).
pub(super) fn deactivate() {
    // SAFETY: `deactivate` takes no argument and returns nothing.
    let _: () = unsafe { msg_send![&*app(), deactivate] };
}

/// Whether one wait for the app to become active is set, so a second press never stacks another.
static AWAITING_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// The window the wait makes key once the app is active: the latest asked for, so a wait
    /// that outlives its press never gives the keys to a window the person has left. The main
    /// thread's alone, as the windows are.
    static WANTED: std::sync::Mutex<Option<Retained<AnyObject>>> = const { std::sync::Mutex::new(None) };
}

/// Make `window` the one the wait makes key (none: the wait has nothing left to do).
fn want(window: Option<&AnyObject>) {
    // SAFETY: `window` is a live NSWindow; retaining it keeps it until the wait ends.
    let window = window
        .and_then(|window| unsafe { Retained::retain(std::ptr::from_ref(window).cast_mut()) });
    WANTED.with(|wanted| *crate::lock(wanted) = window);
}

/// llx-desk the active app and `window` its key window, for the person's press. The app becomes
/// active only later (the system answers the request on a later turn), so the window is made key
/// again once it has: done at once, it would be key in an app still behind another.
pub(super) fn activate_and_key(window: &AnyObject) {
    use std::sync::atomic::Ordering;
    // SAFETY: NSApplication answers whether it is the active app, a BOOL.
    let active: Bool = unsafe { msg_send![&*app(), isActive] };
    if active.as_bool() {
        // The wait still pending is for a window the person has left now.
        want(None);
        // SAFETY: an NSWindow answers whether it is the key window, a BOOL.
        let key: Bool = unsafe { msg_send![window, isKeyWindow] };
        if !key.as_bool() {
            // SAFETY: `makeKeyAndOrderFront:` takes a sender, which may be nil, and returns nothing.
            let _: () =
                unsafe { msg_send![window, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()] };
        }
        return;
    }
    // The latest press is the window the keys go to; one wait serves them all.
    want(Some(window));
    if !AWAITING_ACTIVE.swap(true, Ordering::AcqRel) {
        when_active();
    }
    activate();
    // SAFETY: `makeKeyWindow` takes no argument and returns nothing.
    let _: () = unsafe { msg_send![window, makeKeyWindow] };
}

/// The window asked for last made key once, when the app next becomes active; the wait then ends.
fn when_active() {
    use block2::RcBlock;
    use std::sync::atomic::Ordering;
    // SAFETY: `defaultCenter` is NSNotificationCenter's class method, answering the shared center.
    let center: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNotificationCenter), defaultCenter] };
    let observer = std::sync::Arc::new(std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()));
    let held = std::sync::Arc::clone(&observer);
    let block = RcBlock::new(move |_note: *mut AnyObject| {
        let window = WANTED.with(|wanted| crate::lock(wanted).take());
        if let Some(window) = window {
            // SAFETY: `makeKeyAndOrderFront:` takes a sender, which may be nil, and returns nothing.
            let _: () =
                unsafe { msg_send![&*window, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()] };
        }
        let mine = held.swap(std::ptr::null_mut(), Ordering::AcqRel);
        // SAFETY: the observer the center answered below, retained there; removed once, then
        // released.
        if let Some(mine) = unsafe { Retained::from_raw(mine) } {
            // SAFETY: the shared center; an observer it gave.
            let center: Retained<AnyObject> =
                unsafe { msg_send![class!(NSNotificationCenter), defaultCenter] };
            // SAFETY: `removeObserver:` takes the observer and returns nothing.
            let _: () = unsafe { msg_send![&*center, removeObserver: &*mine] };
        }
        AWAITING_ACTIVE.store(false, Ordering::Release);
    });
    let name = text("NSApplicationDidBecomeActiveNotification");
    // SAFETY: `mainQueue` is NSOperationQueue's class method: the block runs on the main thread.
    let queue: Retained<AnyObject> = unsafe { msg_send![class!(NSOperationQueue), mainQueue] };
    // SAFETY: a name, a nil object, a queue and a block of `void (^)(NSNotification *)`, which
    // the center copies; it answers the observer (autoreleased, retained here).
    let made: Option<Retained<AnyObject>> = unsafe {
        msg_send![&*center, addObserverForName: &*name, object: std::ptr::null::<AnyObject>(), queue: &*queue, usingBlock: &*block]
    };
    match made {
        Some(made) => observer.store(Retained::into_raw(made), Ordering::Release),
        None => AWAITING_ACTIVE.store(false, Ordering::Release),
    }
}

/// The shared workspace (NSWorkspace): what opens files and addresses as the system does.
fn workspace() -> Retained<AnyObject> {
    // SAFETY: NSWorkspace's class method answers the shared workspace.
    unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] }
}

/// `address` opened by the app the person chose for it (their default browser, for a web
/// page); whether the system took it.
pub(super) fn open_outside(address: &str) -> bool {
    let Some(text) = string(address) else {
        return false;
    };
    // SAFETY: NSURL's class method takes an NSString and answers an NSURL, or nil when the
    // text is no address.
    let url: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(NSURL), URLWithString: &*text] };
    let Some(url) = url else {
        return false;
    };
    // SAFETY: the workspace opens an NSURL, answering a BOOL.
    let opened: Bool = unsafe { msg_send![&*workspace(), openURL: &*url] };
    opened.as_bool()
}

/// The file or folder at `path` shown, selected, in the Finder; whether it was.
pub(super) fn reveal(path: &std::path::Path) -> bool {
    let (path, root) = (text(&path.to_string_lossy()), text(""));
    // SAFETY: the workspace takes two NSStrings (the path, and "" for a new Finder window) and
    // answers a BOOL.
    let shown: Bool =
        unsafe { msg_send![&*workspace(), selectFile: &*path, inFileViewerRootedAtPath: &*root] };
    shown.as_bool()
}
