//! How a tab's WebView is built: the probe, the handlers that tell the host of its loads, titles
//! and new windows, and on macOS the configuration all tabs share.

use std::sync::Arc;

use tao::event_loop::EventLoopProxy;
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use super::guard::{allowed, elsewhere, local};
use super::host::{Cmd, PROBE, Said, Shared, Window};
use super::platform::user_agent;
use super::popup::{self, Parent};
use super::tabs::BLANK;

pub(super) fn builder<'a>(
    id: u32,
    url: Option<&'a str>,
    window: &Window,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
    parent: Option<Parent>,
) -> WebViewBuilder<'a> {
    let proxy = proxy.clone();
    // Where the tabs share a configuration, the first tab's IPC handler serves them all: a
    // second one under the same name raises an Objective-C exception, which a binary built
    // with `panic = "abort"` cannot catch.
    let with_ipc = id == 1 || !SHARED;
    let (opened, popups, made) = (window.clone(), Arc::clone(shared), proxy.clone());
    let ipc = Arc::clone(shared);
    let told = proxy.clone();
    let loads = Arc::clone(shared);
    let requests = Arc::clone(shared);
    let titles = Arc::clone(shared);
    // A title or a load changes what the strip names the tab.
    let (titled, loaded) = (proxy.clone(), proxy.clone());
    // The person's sign-ins stay, as in their own browser, in the window they see: WebKit's
    // store of the app's own (macOS: ~/Library/WebKit and HTTPStorages, under the binary's
    // name). A window nobody sees (headless, CI, the tests' unseen ones) remembers nothing.
    let forget = window.headless || super::platform::unseen();
    // A window nobody sees sends the person's browser nothing.
    let headless = window.headless;
    super::frames::begin();
    let mut builder = WebViewBuilder::new()
        // The first click in a window that is not the key one reaches the page: the person
        // never clicks twice, once to wake the window and once to do what they meant.
        .with_accept_first_mouse(true)
        // A page asks for the camera or the microphone as in any browser: WebKit asks the person,
        // for that page's address (without a handler it would say yes to every page once the
        // app itself was allowed).
        .with_permission_handler(|_| wry::PermissionResponse::Default)
        .with_incognito(forget)
        .with_initialization_script_for_main_only(
            PROBE
                .replace("__TOKEN__", &shared.token)
                .replace("__CALL_KEY__", &shared.key)
                .replace(
                    "__HEADLESS__",
                    if window.headless { "true" } else { "false" },
                ),
            true,
        )
        .with_on_page_load_handler(move |event, url| {
            super::debug(|| {
                let kind = if matches!(event, PageLoadEvent::Started) {
                    "started"
                } else {
                    "finished"
                };
                format!("tab {id} load {kind}: {url}")
            });
            loads.page_load(id, &event, &url);
            let _ = loaded.send_event(Cmd::Strip);
            if matches!(event, PageLoadEvent::Finished) {
                super::tabs::drawn(&loads, id, &loaded);
            }
        })
        .with_document_title_changed_handler(move |title| {
            titles.titled(id, &title);
            let _ = titled.send_event(Cmd::Strip);
        })
        // The page itself only ever goes to a local address: anywhere else (a link, a script, a
        // server's redirect) opens in the person's browser, and the tab stays where it was.
        .with_navigation_handler(move |url| navigation(id, &url, headless, &requests))
        .with_download_started_handler(|_url, _path| false)
        // A link with target=_blank or `window.open` opens a tab, which is shown: what the page
        // opened is what it is about now. Built while WebKit waits, it keeps its opener
        // (popup.rs); where it cannot be, it opens as a tab of its own.
        .with_new_window_req_handler(move |url, features| {
            if !local(&url) {
                if !headless {
                    elsewhere(&url, Some(id));
                }
                return NewWindowResponse::Deny;
            }
            let made = parent
                .and_then(|parent| popup::open(features, parent, id, &opened, &popups, &made));
            made.unwrap_or_else(|| {
                let opener = Some(id);
                let _ = proxy.send_event(Cmd::NewTab {
                    url,
                    reply: None,
                    opener,
                });
                NewWindowResponse::Deny
            })
        })
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled);
    // A popup's view loads nothing of its own: WebKit loads the page's request into it.
    if let Some(url) = url {
        builder = builder.with_url(url);
    }
    if with_ipc {
        // A tab's own handler knows its tab; the one shared by all knows none.
        let from = (!SHARED).then_some(id);
        builder = builder.with_ipc_handler(move |request| {
            let command = match ipc.ipc(request.body(), from) {
                Some(Said::Noted(noted)) => Cmd::Noted(noted),
                Some(Said::Close(id)) => {
                    // Nobody waits for the answer.
                    let (reply, _) = std::sync::mpsc::sync_channel(1);
                    Cmd::CloseTab { id, reply }
                }
                None => return,
            };
            let _ = told.send_event(command);
        });
    }
    if let Some(agent) = user_agent() {
        builder = builder.with_user_agent(agent);
    }
    builder
}

/// Whether tab `id` goes to `url`. A frame inside the page goes to any address a tab may load;
/// the page itself only to a local one, and anywhere else opens in the person's browser.
fn navigation(id: u32, url: &str, headless: bool, shared: &Shared) -> bool {
    super::frames::begin();
    let main = super::frames::main_frame();
    let allow = match main {
        Some(false) => allowed(url),
        _ => local(url) || url == BLANK,
    };
    if !allow && main.is_some() && !headless {
        elsewhere(url, Some(id));
    }
    super::debug(|| format!("tab {id} navigation requested: {url} (allowed: {allow})"));
    if allow {
        shared.navigation_requested(id);
    }
    allow
}

/// Whether the tabs share one WebKit configuration (see `share`).
pub(super) const SHARED: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
thread_local! {
    /// The app's first page's configuration: every page of every window shares it.
    static FIRST: std::sync::Mutex<Option<objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration>>> =
        const { std::sync::Mutex::new(None) };
}

/// On macOS a new tab shares the app's first page's configuration, in whichever window: one
/// cookie store, as in a browser (a page opened in a new tab or window stays signed in), and one
/// IPC channel (the first page's view serves it).
#[cfg(target_os = "macos")]
pub(super) fn share<'a>(
    builder: WebViewBuilder<'a>,
    first: Option<&WebView>,
) -> WebViewBuilder<'a> {
    use wry::WebViewBuilderExtMacos;
    let kept = FIRST.with(|kept| crate::lock(kept).clone());
    match kept.or_else(|| first.map(configuration)) {
        Some(configuration) => builder.with_webview_configuration(configuration),
        None => builder,
    }
}

/// `view`'s WebKit configuration.
#[cfg(target_os = "macos")]
fn configuration(view: &WebView) -> objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration> {
    use objc2::msg_send;
    use wry::WebViewExtMacOS;
    let view = view.webview();
    // SAFETY: the WKWebView is alive (borrowed); `configuration` answers a copy of its
    // WKWebViewConfiguration, retained.
    unsafe { msg_send![&*view, configuration] }
}

/// `first` is the app's first page (its pages start over): the pages after it share its
/// configuration.
#[cfg(target_os = "macos")]
pub(super) fn remember(first: &WebView) {
    let configuration = configuration(first);
    FIRST.with(|kept| *crate::lock(kept) = Some(configuration));
}

/// The app has no page: the next one is the first again.
#[cfg(target_os = "macos")]
pub(super) fn forget() {
    FIRST.with(|kept| *crate::lock(kept) = None);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn share<'a>(
    builder: WebViewBuilder<'a>,
    _first: Option<&WebView>,
) -> WebViewBuilder<'a> {
    builder
}

#[cfg(not(target_os = "macos"))]
pub(super) fn remember(_first: &WebView) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn forget() {}

/// `builder` for one of the app's own pages (the strip's parts, a panel): remembering
/// nothing, never taking the keyboard, drawing while the person works elsewhere, its own
/// document only (it goes nowhere else, opens no window, and downloads nothing).
pub(super) fn own_page(builder: WebViewBuilder) -> WebViewBuilder {
    use std::sync::atomic::{AtomicBool, Ordering};
    let first = AtomicBool::new(true);
    builder
        .with_incognito(true)
        .with_focused(false)
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
}

/// One of the app's own pages (`own_page`), built in `window`: it takes the system's appearance
/// as the system's own web interfaces do, so its accent, selection and focus colours are the
/// person's (`platform::system_appearance`).
pub(super) fn build_own(
    builder: WebViewBuilder,
    window: &tao::window::Window,
) -> wry::Result<WebView> {
    let webview = build(builder, window)?;
    super::platform::system_appearance(&webview);
    Ok(webview)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<WebView> {
    builder.build_as_child(window)
}

#[cfg(target_os = "linux")]
pub(super) fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<WebView> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let vbox = (window.default_vbox())
        .ok_or_else(|| std::io::Error::other("the window has no box to put a page in"))?;
    builder.build_gtk(vbox)
}
