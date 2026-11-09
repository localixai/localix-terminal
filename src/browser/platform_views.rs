//! Native WebView rendering, parenting and layout; the platform facade exposes the common surface.

/// WebKit stops animation frames and timers in a window it considers hidden, and a driven window
/// usually sits behind the terminal that started it. So occlusion detection, a WKWebView switch,
/// is off ([`keep_tab_rendering`]). A headless window is fully transparent, passes clicks through,
/// and goes behind every window, as Bun's and WebKit's own test windows do: on screen, so WebKit
/// sees it shown and the display it is on stays awake, under whatever the person watches. Only
/// where the switch is missing does it float, so that nothing covers it. (A shown window is
/// concealed the same way while the person has it put away: nspanel.rs.)
#[cfg(target_os = "macos")]
pub(in super::super) fn keep_rendering(webview: &wry::WebView) {
    use wry::WebViewExtMacOS;
    let switched = keep_tab_rendering(webview);
    let window = webview.ns_window();
    let rendering = if switched {
        super::super::nspanel::Rendering::Switched
    } else {
        super::super::nspanel::Rendering::Floating
    };
    super::super::nspanel::conceal_object(window.as_ref(), rendering);
}

/// A page keeps drawing while covered: WebKit's occlusion detection is off for its WebView.
/// Whether it is: the switch is private, and a later WebKit may not have it.
#[cfg(target_os = "macos")]
pub(in super::super) fn keep_tab_rendering(webview: &wry::WebView) -> bool {
    use objc2::runtime::Bool;
    use objc2::{msg_send, sel};
    use wry::WebViewExtMacOS;
    let view = webview.webview();
    let switch = sel!(_setWindowOcclusionDetectionEnabled:);
    // SAFETY: the WKWebView is a live NSObject; `respondsToSelector:` takes a selector and
    // answers a BOOL.
    let available: bool = unsafe { msg_send![&*view, respondsToSelector: switch] };
    if available {
        // SAFETY: the view answers this private selector (checked just above); it takes a BOOL.
        let _: () = unsafe { msg_send![&*view, _setWindowOcclusionDetectionEnabled: Bool::NO] };
    }
    // WebKit decides whether the page is shown when the view joins its window, with the switch
    // still on: a window that opened unseen (the screen asleep, another Space) stayed hidden,
    // its animation frames stopped. Hidden and shown again, it looks once more.
    // SAFETY: an NSView takes a BOOL `hidden`.
    let _: () = unsafe { msg_send![&*view, setHidden: Bool::YES] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*view, setHidden: Bool::NO] };
    available
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn keep_tab_rendering(_webview: &wry::WebView) -> bool {
    false
}

/// Whether `webview` lets its window show through where its page draws nothing: a new tab's
/// view does until its page drew (tabs::see_through_until_drawn). WebKit's switch for it is
/// private, as wry's own transparency uses it; a WebKit without it draws white as before.
#[cfg(target_os = "macos")]
pub(in super::super) fn see_through(webview: &wry::WebView, through: bool) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use wry::WebViewExtMacOS;
    let view = webview.webview();
    let key = super::super::appkit::text("drawsBackground");
    // SAFETY: NSNumber's class method takes a BOOL and returns an autoreleased NSNumber.
    let draws: *mut AnyObject =
        unsafe { msg_send![objc2::class!(NSNumber), numberWithBool: Bool::new(!through)] };
    // SAFETY: the WKWebView is a live NSObject; key-value coding takes an object and an NSString
    // key, and WKWebView answers `drawsBackground` (wry sets it the same way).
    let _: () = unsafe { msg_send![&*view, setValue: draws, forKey: &*key] };
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn see_through(_webview: &wry::WebView, _through: bool) {}

/// `webview`, one of the app's own pages, in the system's appearance: WebKit then gives its
/// pages the person's accent (`-apple-system-control-accent`), the system's selection, link and
/// focus ring colours, and follows the view's light or dark, where otherwise it answers fixed
/// values so a site cannot read them. The switch is private, as the system's own web interfaces
/// use it; a WebKit without it keeps the fixed values. Never for a site's page.
#[cfg(target_os = "macos")]
pub(in super::super) fn system_appearance(webview: &wry::WebView) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use wry::WebViewExtMacOS;
    let view = webview.webview();
    let object: &AnyObject = &view;
    let selector = objc2::sel!(_setUseSystemAppearance:);
    // SAFETY: `respondsToSelector:` takes a selector and answers a BOOL.
    let takes: Bool = unsafe { msg_send![object, respondsToSelector: selector] };
    if takes.as_bool() {
        // SAFETY: the WKWebView answers this private selector (checked just above); a BOOL.
        let _: () = unsafe { msg_send![object, _setUseSystemAppearance: Bool::YES] };
    }
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn system_appearance(_webview: &wry::WebView) {}

/// How a view follows its window while the person drags the window's edge. (A GTK box lays its
/// views out itself.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Follow {
    /// A page or a terminal: as wide and as tall as the window, under the strip.
    #[cfg(not(target_os = "linux"))]
    Fill,
    /// The strip: as wide as the window, its height kept, held to the window's top.
    Top,
    /// A page of a size of its own: that size kept, in the middle of the window.
    #[cfg(not(target_os = "linux"))]
    Centre,
    /// The side: its width kept, as tall as the window under the strip, held to its left.
    Side,
}

/// `native` clear, or seen again: the window a carried tab is in, over another window's list.
#[cfg(target_os = "macos")]
pub(in super::super) fn clear(native: &tao::window::Window, clear: bool) {
    use objc2::msg_send;
    let alpha: f64 = if clear { 0.0 } else { 1.0 };
    // SAFETY: an NSWindow takes a CGFloat.
    let _: () = unsafe { msg_send![super::super::appkit::native(native), setAlphaValue: alpha] };
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn clear(_native: &tao::window::Window, _clear: bool) {}

/// `native` over the app's other windows, without taking the keys.
#[cfg(target_os = "macos")]
pub(in super::super) fn to_front(native: &tao::window::Window) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let none: *const AnyObject = std::ptr::null();
    // SAFETY: an NSWindow's `orderFront:` takes a sender, which may be nil.
    let _: () = unsafe { msg_send![super::super::appkit::native(native), orderFront: none] };
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn to_front(native: &tao::window::Window) {
    native.set_visible(true);
}

/// Move `webview` into `native`'s content, as it is: a tab's page or panel going to another
/// window goes on (no reload, its program running), and lays out as that window's children do.
#[cfg(target_os = "macos")]
pub(in super::super) fn reparent(webview: &wry::WebView, native: &tao::window::Window) {
    use tao::platform::macos::WindowExtMacOS;
    use wry::WebViewExtMacOS;
    if let Err(why) = webview.reparent(native.ns_window().cast()) {
        log::warn!("a tab's view could not move to its window: {why}");
    }
}

/// Move `webview` into `native`'s box (GTK), as it is.
#[cfg(target_os = "linux")]
pub(in super::super) fn reparent(webview: &wry::WebView, native: &tao::window::Window) {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewExtUnix;
    if let Some(vbox) = native.default_vbox()
        && let Err(why) = webview.reparent(vbox)
    {
        log::warn!("a tab's view could not move to its window: {why}");
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(in super::super) fn reparent(_webview: &wry::WebView, _native: &tao::window::Window) {}

/// AppKit resizes `webview` with its window in the same pass as the window itself: the window's
/// own resize event comes after the frame is drawn, and a view laid out only then leaves the
/// new part of the window empty for a frame. The layout that event brings corrects the rest.
#[cfg(target_os = "macos")]
pub(in super::super) fn follow(webview: &wry::WebView, follow: Follow) {
    use objc2::msg_send;
    use wry::WebViewExtMacOS;
    /// NSAutoresizingMaskOptions.
    const LEFT: usize = 1;
    const WIDTH: usize = 2;
    const RIGHT: usize = 4;
    const BELOW: usize = 8;
    const HEIGHT: usize = 16;
    const ABOVE: usize = 32;
    let mask = match follow {
        Follow::Fill => WIDTH | HEIGHT,
        Follow::Top => WIDTH | BELOW,
        Follow::Centre => LEFT | RIGHT | BELOW | ABOVE,
        Follow::Side => HEIGHT,
    };
    let view = webview.webview();
    // SAFETY: an NSView answers its NSAutoresizingMaskOptions (an NSUInteger).
    let now: usize = unsafe { msg_send![&*view, autoresizingMask] };
    // Every layout says it again: one already following so is left alone.
    if now != mask {
        // SAFETY: an NSView takes an NSAutoresizingMaskOptions (an NSUInteger).
        let _: () = unsafe { msg_send![&*view, setAutoresizingMask: mask] };
    }
}

/// GTK owns its children's layout; elsewhere there is no native autoresizing mask to set.
#[cfg(not(target_os = "macos"))]
pub(in super::super) fn follow(_webview: &wry::WebView, _follow: Follow) {}

/// The glass under the strip's open list, and the strip over every page (pane.rs).
#[cfg(target_os = "macos")]
pub(in super::super) use super::super::pane::{Pane, stack};

/// Elsewhere the strip cannot be put over the pages: they stand aside while a list is open.
#[cfg(not(target_os = "macos"))]
pub(in super::super) fn stack(_views: &[&wry::WebView]) -> bool {
    false
}

/// Elsewhere the list draws its own ground.
#[cfg(not(target_os = "macos"))]
pub(in super::super) struct Pane;

#[cfg(not(target_os = "macos"))]
#[expect(
    clippy::unused_self,
    reason = "the macOS glass's methods, with nothing to keep"
)]
impl Pane {
    pub(in super::super) fn under(_strip: &wry::WebView) -> Option<Self> {
        None
    }

    pub(in super::super) fn place(
        &self,
        _strip: &wry::WebView,
        _rect: Option<(u32, u32, u32, u32)>,
    ) {
    }
}

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn keep_rendering(_webview: &wry::WebView) {}
