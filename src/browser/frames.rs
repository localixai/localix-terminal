//! Whether the navigation a tab's handler decides now is of the page itself or of a frame in it.
//! Only the page is held to a local address; its frames (an embedded map, a video, a sign-in
//! widget) load from anywhere, as its scripts and pictures do. The view's handler is told only the
//! address, so on macOS WebKit's question is looked at on its way to the handler: its navigation
//! action names the frame it targets, and that is noted while the handler answers.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the navigation decided now targets the page's main frame. Only the main thread sets
/// and reads it: WebKit asks there, and the handler answers there, at once.
static MAIN: AtomicBool = AtomicBool::new(true);

/// Whether the navigation a tab's handler decides now is the page's own; none where it cannot be
/// told (elsewhere than macOS, or before the first view was built).
pub(super) fn main_frame() -> Option<bool> {
    known().then(|| MAIN.load(Ordering::Relaxed))
}

#[cfg(target_os = "macos")]
mod watch {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{msg_send, sel};

    /// `webView:decidePolicyForNavigationAction:decisionHandler:` as the view's delegate has it.
    type Decide = unsafe extern "C-unwind" fn(
        *mut AnyObject,
        Sel,
        *mut AnyObject,
        *mut AnyObject,
        *mut AnyObject,
    );

    static WATCHED: AtomicBool = AtomicBool::new(false);
    static ORIGINAL: OnceLock<Decide> = OnceLock::new();

    /// Whether `action` (a WKNavigationAction) targets its page's main frame; a new window's
    /// (no frame yet) is a page of its own.
    fn of_main(action: &AnyObject) -> bool {
        // SAFETY: a navigation action answers its target frame, a WKFrameInfo, or nil.
        let frame: Option<Retained<AnyObject>> = unsafe { msg_send![action, targetFrame] };
        // SAFETY: a frame's info answers whether it is the main frame, a BOOL.
        frame.is_none_or(|frame| unsafe { msg_send![&*frame, isMainFrame] })
    }

    /// The delegate's answer, with the frame it is about noted around it.
    unsafe extern "C-unwind" fn decide(
        this: *mut AnyObject,
        selector: Sel,
        view: *mut AnyObject,
        action: *mut AnyObject,
        handler: *mut AnyObject,
    ) {
        // SAFETY: WebKit passes a live WKNavigationAction, or nothing.
        let main = unsafe { action.as_ref() }.is_none_or(of_main);
        let before = super::MAIN.swap(main, Ordering::Relaxed);
        if let Some(original) = ORIGINAL.get() {
            // SAFETY: the delegate's own method, called with what WebKit gave this one.
            unsafe { original(this, selector, view, action, handler) };
        }
        super::MAIN.store(before, Ordering::Relaxed);
    }

    /// Whether the delegate's question is looked at.
    pub(in super::super) fn known() -> bool {
        WATCHED.load(Ordering::Relaxed)
    }

    /// The tabs' delegate's question is looked at from now on: its class is `view`'s delegate's
    /// (wry's, named by its module and version, so it is taken from a built view, never by name).
    /// On the main thread, where WebKit asks; once, as every tab has a delegate of that class.
    pub(in super::super) fn watch(view: &wry::WebView) {
        use wry::WebViewExtMacOS;
        if WATCHED.load(Ordering::Relaxed) {
            return;
        }
        let view = view.webview();
        // SAFETY: a WKWebView answers its navigation delegate, or nil.
        let delegate: Option<Retained<AnyObject>> =
            unsafe { msg_send![&*view, navigationDelegate] };
        let Some(delegate) = delegate else {
            return;
        };
        let class: &AnyClass = delegate.class();
        let selector = sel!(webView:decidePolicyForNavigationAction:decisionHandler:);
        let Some(method) = class.instance_method(selector) else {
            return;
        };
        // SAFETY: the method's implementation has `Decide`'s signature (an object, its selector,
        // the view, the action and the handler block).
        let original = unsafe { std::mem::transmute::<Imp, Decide>(method.implementation()) };
        if ORIGINAL.set(original).is_err() {
            return;
        }
        WATCHED.store(true, Ordering::Relaxed);
        // SAFETY: `decide` has the same signature, and calls the implementation it replaces.
        let ours = unsafe { std::mem::transmute::<Decide, Imp>(decide) };
        // SAFETY: as above.
        unsafe { method.set_implementation(ours) };
    }
}

/// From now on a tab's handler knows which frame it decides for (`main_frame`); elsewhere than
/// macOS it never does.
#[cfg(target_os = "macos")]
pub(super) use watch::{known, watch};

#[cfg(not(target_os = "macos"))]
pub(super) fn watch(_view: &wry::WebView) {}

#[cfg(not(target_os = "macos"))]
fn known() -> bool {
    false
}
