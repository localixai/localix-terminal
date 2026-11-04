//! What differs by platform: where a headless window goes, keeping it rendering, the Dock, the user agent.

use tao::event_loop::EventLoopWindowTarget;
use tao::window::WindowBuilder;

/// With `DESK_WINDOWS=unseen` a shown window is everything it is, strip,
/// glass and panels, but on screen it is what a headless one is: clear, passing clicks through,
/// with no Dock icon, so a test run never covers what the person is watching (macOS). Its pages
/// remember nothing, anywhere (tab_view.rs).
pub(super) fn unseen() -> bool {
    std::env::var_os("DESK_WINDOWS").is_some_and(|value| value == "unseen")
}

/// Whether a window shows, so the app is an accessory one: no Dock icon and no menu bar (the mini
/// replaces the icon's role), and never activated by anything llx-desk does. Otherwise (no window shown,
/// or only a headless one) it is a background app macOS never activates: an accessory one was, at
/// launch, by a process started from the terminal the person types in (a test, `exec`,
/// `--no-daemon`), and their keys went to it, so the policy changes only once a window shows.
#[cfg(target_os = "macos")]
pub(super) fn dock<T>(target: &EventLoopWindowTarget<T>, shown: bool) {
    use tao::platform::macos::{ActivationPolicy, EventLoopWindowTargetExtMacOS};
    let policy = match shown && !unseen() {
        true => ActivationPolicy::Accessory,
        false => ActivationPolicy::Prohibited,
    };
    target.set_activation_policy_at_runtime(policy);
    if shown {
        super::edit_menu::install();
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn dock<T>(_target: &EventLoopWindowTarget<T>, _shown: bool) {}

/// The primary screen's room for windows: below the menu bar and above the Dock on macOS, else
/// the whole monitor. `None` when there is no monitor.
pub(super) fn work_area<T>(target: &EventLoopWindowTarget<T>) -> Option<super::placement::Area> {
    #[cfg(target_os = "macos")]
    if let Some(area) = super::nspanel::work_area() {
        return Some(area);
    }
    let monitor = target.primary_monitor()?;
    let scale = monitor.scale_factor();
    let (size, at) = (
        monitor.size().to_logical::<f64>(scale),
        monitor.position().to_logical::<f64>(scale),
    );
    Some(super::placement::Area {
        x: at.x,
        y: at.y,
        width: size.width,
        height: size.height,
    })
}

/// The mini's window as a floating panel on every Space, never taking the keyboard.
#[cfg(target_os = "macos")]
pub(super) fn adopt_mini(window: &tao::window::Window) {
    super::nspanel::adopt(window, super::nspanel::Role::Mini);
}

/// The deck's minimize button collapses it to the mini.
#[cfg(target_os = "macos")]
pub(super) fn collapse_button(
    window: &tao::window::Window,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
    super::nspanel::collapse_to(window, proxy);
}

/// Elsewhere there is no mini: the button minimizes as the system does.
#[cfg(not(target_os = "macos"))]
pub(super) fn collapse_button(
    _window: &tao::window::Window,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
}

/// The mini's own grip over its page: a press moves it, a click expands it.
#[cfg(target_os = "macos")]
pub(super) fn grip_mini(
    window: &tao::window::Window,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
    super::mini_grip::grip(window, proxy);
}

#[cfg(target_os = "macos")]
pub(super) fn show_mini(window: &tao::window::Window) {
    super::nspanel::reveal(
        window,
        super::nspanel::Role::Mini,
        super::nspanel::Rendering::Switched,
    );
}

#[cfg(target_os = "macos")]
pub(super) fn hide_mini(window: &tao::window::Window) {
    super::nspanel::hide(window);
}

/// The person's moves between Spaces, told to the loop: the mini is put on every Space again.
#[cfg(target_os = "macos")]
pub(super) fn watch_spaces(proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {
    super::nspanel::watch_spaces(proxy);
}

/// Elsewhere there are no Spaces to watch.
#[cfg(not(target_os = "macos"))]
pub(super) fn watch_spaces(_proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn adopt_mini(_window: &tao::window::Window) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn grip_mini(
    _window: &tao::window::Window,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
}

#[cfg(not(target_os = "macos"))]
pub(super) fn show_mini(window: &tao::window::Window) {
    window.set_visible(true);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn hide_mini(window: &tao::window::Window) {
    window.set_visible(false);
}

/// Where a headless window goes. macOS keeps it on screen and makes it invisible afterwards
/// ([`keep_rendering`]): WebKit marks an off-screen window hidden and stops its animation frames
/// and timers, so a page there would never settle. Elsewhere it goes off screen.
#[cfg(target_os = "macos")]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    builder
}

#[cfg(not(target_os = "macos"))]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    let size = super::viewport::Viewport::DESKTOP;
    builder.with_position(tao::dpi::LogicalPosition::new(
        -size.width * 4.0,
        -size.height * 4.0,
    ))
}

/// WebKit stops animation frames and timers in a window it considers hidden, and a driven window
/// usually sits behind the terminal that started it. So occlusion detection, a WKWebView switch,
/// is off ([`keep_tab_rendering`]). A headless window is fully transparent, passes clicks through,
/// and goes behind every window, as Bun's and WebKit's own test windows do: on screen, so WebKit
/// sees it shown and the display it is on stays awake, under whatever the person watches. Only
/// where the switch is missing does it float, so that nothing covers it. (A shown window is
/// concealed the same way while it is the mini: nspanel.rs.)
#[cfg(target_os = "macos")]
pub(super) fn keep_rendering(webview: &wry::WebView) {
    use wry::WebViewExtMacOS;
    let switched = keep_tab_rendering(webview);
    let window = webview.ns_window();
    let rendering = if switched {
        super::nspanel::Rendering::Switched
    } else {
        super::nspanel::Rendering::Floating
    };
    super::nspanel::conceal_object(window.as_ref(), rendering);
}

/// A page keeps drawing while covered: WebKit's occlusion detection is off for its WebView.
/// Whether it is: the switch is private, and a later WebKit may not have it.
#[cfg(target_os = "macos")]
pub(super) fn keep_tab_rendering(webview: &wry::WebView) -> bool {
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
pub(super) fn keep_tab_rendering(_webview: &wry::WebView) -> bool {
    false
}

/// How a view follows its window while the person drags the window's edge. (A GTK box lays its
/// views out itself.)
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Follow {
    /// A page or a terminal: as wide and as tall as the window, under the strip.
    Fill,
    /// The strip: as wide as the window, its height kept, held to the window's top.
    Top,
    /// A page of a size of its own: that size kept, in the middle of the window.
    Centre,
}

/// AppKit resizes `webview` with its window in the same pass as the window itself: the window's
/// own resize event comes after the frame is drawn, and a view laid out only then leaves the
/// new part of the window empty for a frame. The layout that event brings corrects the rest.
#[cfg(target_os = "macos")]
pub(super) fn follow(webview: &wry::WebView, follow: Follow) {
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
    };
    let view = webview.webview();
    // SAFETY: an NSView takes an NSAutoresizingMaskOptions (an NSUInteger).
    let _: () = unsafe { msg_send![&*view, setAutoresizingMask: mask] };
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn follow(_webview: &wry::WebView, _follow: Follow) {}

/// The person picks one of `items` from the system's menu at `at` (menu.rs).
#[cfg(target_os = "macos")]
pub(super) fn choose(
    strip: &wry::WebView,
    at: (f64, f64),
    items: &[&str],
    marked: Option<usize>,
) -> Option<usize> {
    super::menu::choose(strip, at, items, marked)
}

/// Elsewhere GTK's menu, at the pointer (the click that asked for it is under it).
#[cfg(not(target_os = "macos"))]
pub(super) fn choose(
    strip: &wry::WebView,
    _at: (f64, f64),
    items: &[&str],
    marked: Option<usize>,
) -> Option<usize> {
    super::menu_gtk::choose(strip, items, marked)
}

/// The pasteboard's text (pasteboard.rs); elsewhere none yet.
#[cfg(target_os = "macos")]
pub(super) fn pasted() -> Option<String> {
    super::pasteboard::pasted()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn pasted() -> Option<String> {
    None
}

/// Put `text` on the pasteboard; elsewhere nothing yet.
#[cfg(target_os = "macos")]
pub(super) fn copy(text: &str) {
    super::pasteboard::copy(text);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn copy(_text: &str) {}

/// The blur under the strip, over the pages that run under it (under.rs).
#[cfg(target_os = "macos")]
pub(super) use super::under::Band;

/// The glass under the strip's open list (pane.rs).
#[cfg(target_os = "macos")]
pub(super) use super::pane::Pane;

/// Elsewhere the list draws its own ground.
#[cfg(not(target_os = "macos"))]
pub(super) struct Pane;

#[cfg(not(target_os = "macos"))]
#[expect(
    clippy::unused_self,
    reason = "the macOS glass's methods, with nothing to keep"
)]
impl Pane {
    pub(super) fn under(_strip: &wry::WebView) -> Option<Self> {
        None
    }

    pub(super) fn place(&self, _strip: &wry::WebView, _rect: Option<(u32, u32, u32, u32)>) {}

    pub(super) fn shade(&self, _dark: Option<bool>) {}
}

/// Elsewhere no page runs under the strip, and nothing is blurred.
#[cfg(not(target_os = "macos"))]
pub(super) struct Band;

#[cfg(not(target_os = "macos"))]
#[expect(
    clippy::unused_self,
    reason = "the macOS blur's methods, with nothing to keep"
)]
impl Band {
    pub(super) fn behind(_strip: &wry::WebView) -> Option<Self> {
        None
    }

    pub(super) fn lay(&self, _strip: &wry::WebView, _top: f64) {}

    pub(super) fn shade(&self, _dark: Option<bool>) {}
}

/// Tell a page the strip covers its first `top` points; false where it must stay below the strip.
#[cfg(target_os = "macos")]
pub(super) fn inset(webview: &wry::WebView, top: f64) -> bool {
    super::under::inset(webview, top)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn inset(_webview: &wry::WebView, _top: f64) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub(super) fn keep_rendering(_webview: &wry::WebView) {}

/// The installed Safari's user agent. A plain WKWebView omits the `Version/… Safari/…` tokens,
/// and sites such as Google then serve a legacy page.
#[cfg(target_os = "macos")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "one signature on every system: elsewhere there is none"
)]
pub(super) fn user_agent() -> Option<String> {
    let plist = std::fs::read_to_string("/Applications/Safari.app/Contents/Info.plist").ok();
    let version = plist
        .as_deref()
        .and_then(|plist| plist.split("<key>CFBundleShortVersionString</key>").nth(1))
        .and_then(|rest| rest.split("<string>").nth(1))
        .and_then(|rest| rest.split("</string>").next())
        .filter(|version| !version.is_empty() && version.len() < 16)
        .unwrap_or("26.0");
    Some(format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/{version} Safari/605.1.15"
    ))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn user_agent() -> Option<String> {
    None
}
