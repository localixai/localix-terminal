//! The browser: a native window on the system WebView, observed and acted on through a probe
//! script in the page. [`Page`] is the window as a view ([`crate::view::View`]); [`Driver`] is the
//! browser's own, fuller surface (navigate, read, keys, scripts).

mod agents;
#[cfg(target_os = "macos")]
mod app_menu;
#[cfg(target_os = "macos")]
mod appkit;
mod capture;
mod choice;
mod comments;
mod deck;
mod feedback;
mod frame;
mod guard;
mod host;
mod icons;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(not(target_os = "macos"))]
mod menu_gtk;
mod mini;
#[cfg(target_os = "macos")]
mod mini_grip;
mod notices;
#[cfg(target_os = "macos")]
mod nspanel;
mod offer;
mod page;
#[cfg(target_os = "macos")]
mod pane;
mod panel;
#[cfg(target_os = "macos")]
mod pasteboard;
mod placement;
mod platform;
mod popup;
mod presence;
mod scope;
mod script;
mod settings;
mod shortcuts;
mod sizes;
mod strip;
mod tab_view;
mod tabs;
mod told;
mod toolbar;
mod turn;
#[cfg(target_os = "macos")]
mod viewport;
mod webview;

use std::time::Duration;

use crate::view::{Action, Element, ViewResult};
use serde::{Deserialize, Serialize};

pub use capture::{Clip, Shot};
pub use guard::loadable;
pub use host::{
    Answers, Asked, Browser, CenterDid, Confirm, Dressing, Heard, Ledger, Past, Place, Tab,
    TabKind, Visited, Went, Window, host,
};
pub use notices::{Armed, Notice};
pub use page::Page;
pub use panel::{Fitted, Pane, Panel};
pub use strip::BOARD;
pub use strip::HOME;
pub use tabs::MAX_TABS;
pub use viewport::Viewport;
pub use webview::WebviewDriver;

/// Why there is no window where `has_display` is false, as the person and the model read it.
pub const NO_WINDOW: &str = "no display";

/// What would give it one.
pub const NO_WINDOW_HINT: &str = " (run under xvfb-run for it)";

/// Whether a window can be shown here: on Linux, WebKitGTK needs an X or Wayland server.
pub fn has_display() -> bool {
    !cfg!(target_os = "linux")
        || std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// One atomic read of the page, as the probe takes it: the controls a person can see and use, the
/// text around them, and where the page is scrolled.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Observation {
    /// Changes with the document; an action carries it so it never lands on a newer page.
    pub epoch: String,
    pub url: String,
    pub title: String,
    pub text: String,
    /// The page as an outline: headings and text, controls with their refs (`[ref=e3]`).
    #[serde(default)]
    pub outline: String,
    pub viewport: Size,
    pub scroll: Scroll,
    pub elements: Vec<Element>,
    /// The page says it is still working (parsing, fetching its own data, a progress bar).
    #[serde(default)]
    pub busy: bool,
    /// How many times its words or nodes changed, anywhere in the document.
    #[serde(default)]
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Scroll {
    pub y: i64,
    pub height: i64,
}

impl From<Observation> for crate::view::Observation {
    fn from(page: Observation) -> Self {
        Self {
            epoch: page.epoch,
            location: page.url,
            title: page.title,
            text: page.text,
            outline: page.outline,
            elements: page.elements,
            busy: page.busy,
            scroll: page.scroll.y,
            revision: page.revision,
            notices: vec![],
        }
    }
}

/// A browser driven one page at a time: observed and acted on in turn.
pub trait Driver {
    fn navigate(&self, url: &str) -> ViewResult<()>;
    /// Wait until the document is usable (loaded, or parsed while images and frames still load),
    /// up to `timeout`.
    fn wait_loaded(&self, timeout: Duration) -> ViewResult<()>;
    fn observe(&self) -> ViewResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the page settle.
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()>;
    /// Give the page a size of its own, as `size` names it (`iPhone 15 Pro`, `mobile`, `393x852`;
    /// `window` for the whole window), then let it settle; what it is now, in words. Layout only:
    /// the user agent stays.
    fn frame(&self, size: &str) -> ViewResult<String>;
    /// Press `keys` (chords such as "ArrowUp", "Ctrl+A") on the focused element, `delay_ms` apart,
    /// then let the page settle.
    fn press(&self, keys: &[String], delay_ms: u64) -> ViewResult<()>;
    /// Run `source` (JavaScript) in the page and return its value as text, then let the page settle.
    fn evaluate(&self, source: &str) -> ViewResult<String>;
    /// The document's readable text from `offset`, at most `limit` characters.
    fn read(&self, offset: usize, limit: usize) -> ViewResult<PageText>;
    /// A picture of the viewport as drawn, or of the first element matching the CSS `selector`
    /// (scrolled into view first, with a little of its surroundings).
    fn screenshot(&self, selector: Option<&str>) -> ViewResult<Shot>;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageText {
    pub text: String,
    pub total: usize,
    pub offset: usize,
}

/// With `BROWSER_DEBUG` set, `line` on stderr: a driver's timings and navigations, for whoever
/// debugs one.
#[expect(
    clippy::print_stderr,
    reason = "a developer's trace, printed only with BROWSER_DEBUG set (by design)"
)]
fn debug(line: impl FnOnce() -> String) {
    if std::env::var_os("BROWSER_DEBUG").is_some() {
        eprintln!("{}", line());
    }
}
