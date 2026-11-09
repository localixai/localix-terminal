//! What may load in the window, where an address the tabs do not show goes instead, and the
//! per-run token that keeps a page from answering for the probe.
//!
//! A tab's page is always a local address (`local.rs`): an agent asking for another is refused,
//! and one the person or a page goes to opens in the person's own browser.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub(crate) use super::local::local;

/// Only web pages load in a frame; other schemes (files, apps, mail) are refused. `data:` is
/// out too: WebKit drops IPC from its opaque origin, so the probe could never answer there. A
/// page's frames load from anywhere; the page itself only from a local address (`local`).
pub(super) fn allowed(url: &str) -> bool {
    ["http://", "https://", "about:", "blob:"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// `url` as the window can load it: an address the system's URL type takes, spelled so (a space
/// or a letter past ASCII encoded); `None` for what is not an address at all. WebKit's load of
/// an address the system cannot read panics on the main thread, ending the whole process, so
/// nothing reaches a load without passing here.
pub fn loadable(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let web = matches!(parsed.scheme(), "http" | "https");
    if web && parsed.host_str().is_none_or(str::is_empty) {
        return None;
    }
    // What the person or the model wrote stays as written when it needs no spelling out.
    let plain = url.chars().all(|c| c.is_ascii_graphic());
    Some(match plain {
        true => url.to_string(),
        false => parsed.to_string(),
    })
}

/// What an agent is told when it asks a tab for an address that is not local.
pub fn not_local(url: &str) -> String {
    format!(
        "llx-desk's tabs open only local addresses (localhost, *.local, *.test, LAN); \
         this one is external: {url}"
    )
}

/// `url` as an agent's tab loads it: loadable, and local; else why not, before anything loads.
pub fn for_agent(url: &str) -> Result<String, String> {
    let loaded = loadable(url).ok_or_else(|| format!("`{url}` is not an address"))?;
    match local(&loaded) {
        true => Ok(loaded),
        false => Err(not_local(&loaded)),
    }
}

/// 32 hex characters from the system's random source; unguessable to a page in one run.
pub(super) fn token() -> String {
    super::secret::word()
}

/// Whether `url` is an address another browser can open: a web page, or a file.
pub(super) fn outside(url: &str) -> bool {
    ["http://", "https://", "file://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// How often a page may send the person's browser an address: a page that goes away in a loop
/// opens one, not a storm of them.
const PAGE_OPENS_EVERY: Duration = Duration::from_secs(1);

/// When each tab's page last sent an address to the person's browser.
#[derive(Default)]
pub(super) struct Throttle {
    last: HashMap<u32, Instant>,
}

impl Throttle {
    /// Whether tab `tab`'s page may send one now; if so, it is counted.
    pub(super) fn let_through(&mut self, tab: u32, now: Instant) -> bool {
        let recent = (self.last.get(&tab))
            .is_some_and(|last| now.saturating_duration_since(*last) < PAGE_OPENS_EVERY);
        if !recent {
            self.last.insert(tab, now);
        }
        !recent
    }
}

thread_local! {
    /// The main thread's alone: a tab's handlers run there.
    static PAGES: std::sync::Mutex<Throttle> = std::sync::Mutex::default();
}

/// `url`, an address a tab does not show, opened in the person's default browser instead.
/// `from` is the tab whose page went there (a link, a script, a redirect), at most once a second
/// per tab; none is the person's own asking (the address field, a terminal's link).
pub(super) fn elsewhere(url: &str, from: Option<u32>) {
    // Only a web page: a page never has the system open a file or an app.
    if !["http://", "https://"]
        .iter()
        .any(|web| url.starts_with(web))
    {
        return;
    }
    let let_through = match from {
        Some(tab) => PAGES.with(|pages| crate::lock(pages).let_through(tab, Instant::now())),
        None => true,
    };
    super::debug(|| format!("an address elsewhere, from {from:?}: {url} (sent: {let_through})"));
    if let_through {
        send(url);
    }
}

/// The person's default browser given `url`.
#[cfg(not(feature = "test-server"))]
fn send(url: &str) {
    super::platform::open_outside(url);
}

#[cfg(feature = "test-server")]
static SENT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// A test build keeps the address instead: its windows never open the person's browser.
#[cfg(feature = "test-server")]
fn send(url: &str) {
    crate::lock(&SENT).push(url.to_string());
}

/// The addresses a test build's tabs sent to the default browser, oldest first.
#[cfg(feature = "test-server")]
pub fn sent_elsewhere() -> Vec<String> {
    crate::lock(&SENT).clone()
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
