//! A window a page opens (a sign-in popup, a link with target=_blank) as a tab that keeps its
//! opener: WebKit asks for the new view while the page waits, so the tab is built there and then,
//! on the opener's configuration, and handed to WebKit; `window.opener` and `postMessage` work
//! between the two, as a sign-in with Google needs. The tabs take it in at their next turn
//! (`Tabs::adopt`), under the id kept for it here: ids are places, so a tab built meanwhile never
//! takes its place. A popup's `window.close()` reaches the host through the probe and closes it.

use std::sync::Mutex;

use wry::WebView;
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

use super::told::MAX_TABS;

/// The window a tab is built in, by its handle: a page's handler cannot borrow it.
#[derive(Clone, Copy)]
pub(super) struct Parent(RawWindowHandle);

impl Parent {
    pub(super) fn of(native: &tao::window::Window) -> Option<Self> {
        let handle = native.window_handle().ok()?;
        Some(Self(handle.as_raw()))
    }
}

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the window outlives its tabs (the deck drops them first), and only a tab's
        // handler, on the main thread, uses this handle, while that tab lives.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

/// The tabs' places: how many there are, those a closed tab left (the first never: its view
/// serves every tab's IPC), and the popups not yet taken in.
#[derive(Default)]
struct Places {
    taken: u32,
    free: Vec<u32>,
    waiting: Vec<(u32, WebView)>,
}

thread_local! {
    /// The main thread's alone: WebViews never leave it.
    static PLACES: Mutex<Places> = Mutex::default();
}

fn with<T>(act: impl FnOnce(&mut Places) -> T) -> T {
    PLACES.with(|places| act(&mut crate::lock(places)))
}

/// A new window's tabs start with no place taken.
pub(super) fn reset() {
    with(|places| *places = Places::default());
}

/// The id of a new place for a tab: a new one, if there is room.
pub(super) fn reserve() -> Option<u32> {
    with(|places| {
        let room = usize::try_from(places.taken).is_ok_and(|taken| taken < MAX_TABS);
        room.then(|| {
            places.taken += 1;
            places.taken
        })
    })
}

/// The id of a place for a popup: the first a closed tab left, else a new one.
#[cfg_attr(
    not(target_os = "macos"),
    expect(
        dead_code,
        reason = "elsewhere a page's window opens as a tab of its own"
    )
)]
fn reserve_for_popup() -> Option<u32> {
    let left = with(|places| {
        let first = places.free.iter().copied().min()?;
        places.free.retain(|id| *id != first);
        Some(first)
    });
    left.or_else(reserve)
}

/// Tab `id` closed: a popup may take its place.
pub(super) fn free(id: u32) {
    if id != 1 {
        with(|places| places.free.push(id));
    }
}

/// Tab `id`'s place taken again by a tab of the browser's own.
pub(super) fn claim(id: u32) {
    with(|places| places.free.retain(|free| *free != id));
}

/// The popups built since the tabs last looked, in the order of their ids.
pub(super) fn waiting() -> Vec<(u32, WebView)> {
    let mut waiting = with(|places| std::mem::take(&mut places.waiting));
    waiting.sort_unstable_by_key(|(id, _)| *id);
    waiting
}

/// Keep `webview`, built for the place `id`, until the tabs take it in.
#[cfg_attr(
    not(target_os = "macos"),
    expect(
        dead_code,
        reason = "elsewhere a page's window opens as a tab of its own"
    )
)]
pub(super) fn wait(id: u32, webview: WebView) {
    with(|places| places.waiting.push((id, webview)));
}

/// Build the tab a page asked for, on its opener's configuration, as WebKit waits for it: the
/// answer that hands it over, or none (no room, or it could not be built), and the page's window
/// opens as a tab of its own, with no opener.
#[cfg(target_os = "macos")]
pub(super) fn open(
    features: wry::NewWindowFeatures,
    parent: Parent,
    window: &super::host::Window,
    shared: &std::sync::Arc<super::host::Shared>,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) -> Option<wry::NewWindowResponse> {
    use wry::{WebViewBuilderExtMacos, WebViewExtMacOS};
    let id = reserve_for_popup()?;
    let builder = super::tab_view::builder(id, None, window, shared, proxy, None)
        .with_webview_configuration(features.opener.target_configuration);
    let Ok(webview) = builder.build_as_child(&parent) else {
        free(id);
        return None;
    };
    // A WryWebView is a WKWebView: its superclass, as WebKit takes it.
    let view = objc2::rc::Retained::into_super(webview.webview());
    wait(id, webview);
    let _ = proxy.send_event(super::host::Cmd::Adopt);
    Some(wry::NewWindowResponse::Create { webview: view })
}

/// Elsewhere a page's window opens as a tab of its own.
#[cfg(not(target_os = "macos"))]
pub(super) fn open(
    _features: wry::NewWindowFeatures,
    _parent: Parent,
    _window: &super::host::Window,
    _shared: &std::sync::Arc<super::host::Shared>,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) -> Option<wry::NewWindowResponse> {
    None
}
