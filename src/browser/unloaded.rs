//! A page that never came. An address asked of a tab whose document is blank (a new tab, a link
//! opened from a terminal, an address typed into an empty page) may never load: nothing listens
//! there, no such host, a server that does not answer. The view tells of a load that starts and
//! of one that ends, never of one given up, so the tab would stay an empty page that has
//! forgotten its address. Here: what each tab waits for, how it is asked whether its page still
//! comes, and the one line it shows instead (docs/ui.md, an empty place), under the address it
//! keeps, so Reload asks for it again.
//!
//! What came and no page shows (an archive, a program: a file to keep, which the app does not
//! keep) leaves the same empty tab, and gets the same line, said by the file's name.
//!
//! A tab that shows a page is left alone: what is asked of it next may be the page's own or a
//! frame's, and WebKit keeps the page it shows when the next one does not come.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tao::event_loop::EventLoopProxy;

use super::frame;
use super::host::{Cmd, Shared};
use super::tabs::BLANK;

/// What the tabs wait for and keep, by tab.
#[derive(Default)]
pub(super) struct Unloaded {
    tabs: Mutex<HashMap<u32, Slot>>,
}

#[derive(Default)]
struct Slot {
    /// A page's own document is shown.
    page: bool,
    /// The address asked for and the count it was asked at, until it comes or is given up.
    asked: Option<(String, u64)>,
    /// What was asked came, as a file to keep: nothing a page shows.
    apart: bool,
    /// The address that did not come, which the tab says as its own.
    kept: Option<String>,
}

impl Unloaded {
    /// `url` is asked of tab `tab`, the `at`-th thing asked of any tab: whether it is waited
    /// for, which it is while the tab shows no page.
    pub(super) fn asked(&self, tab: u32, url: &str, at: u64) -> bool {
        let mut tabs = crate::lock(&self.tabs);
        let slot = tabs.entry(tab).or_default();
        if slot.page || url == BLANK {
            return false;
        }
        (slot.asked, slot.apart) = (Some((url.to_owned(), at)), false);
        true
    }

    /// What tab `tab` waits for came as a file to keep, which no page shows: the count it was
    /// asked at, to ask the tab now whether its view still loads. None when the tab waits for
    /// nothing (a page's own link to a file: the page stays).
    pub(super) fn kept_apart(&self, tab: u32) -> Option<u64> {
        let mut tabs = crate::lock(&self.tabs);
        let slot = tabs.get_mut(&tab)?;
        let (_, at) = slot.asked.as_ref()?;
        slot.apart = true;
        Some(*at)
    }

    /// Whether tab `tab` still waits for what was asked of it at `at`.
    pub(super) fn waits(&self, tab: u32, at: u64) -> bool {
        let tabs = crate::lock(&self.tabs);
        tabs.get(&tab)
            .and_then(|slot| slot.asked.as_ref())
            .is_some_and(|(_, asked)| *asked == at)
    }

    /// What tab `tab` was asked at `at` did not come: the address, which the tab keeps. None
    /// when it came meanwhile, or something else was asked since.
    pub(super) fn failed(&self, tab: u32, at: u64) -> Option<String> {
        let mut tabs = crate::lock(&self.tabs);
        let slot = tabs.get_mut(&tab)?;
        let (url, _) = slot.asked.take_if(|(_, asked)| *asked == at)?;
        slot.kept = Some(url.clone());
        Some(url)
    }

    /// Tab `tab`'s document is `url` now: the address the tab says. A page's own, and nothing
    /// waits or is kept any more; a blank one's is the address kept, if one did not come.
    pub(super) fn shows(&self, tab: u32, url: &str) -> String {
        let mut tabs = crate::lock(&self.tabs);
        let slot = tabs.entry(tab).or_default();
        slot.page = url != BLANK;
        if slot.page {
            (slot.asked, slot.kept) = (None, None);
        }
        slot.kept.clone().unwrap_or_else(|| url.to_owned())
    }

    /// The address tab `tab` could not load and shows the line for.
    pub(super) fn address(&self, tab: u32) -> Option<String> {
        crate::lock(&self.tabs).get(&tab)?.kept.clone()
    }

    /// The document tab `tab` shows in place of the page it could not: the line for the
    /// address it keeps, which never came or came as nothing a page shows.
    pub(super) fn line(&self, tab: u32) -> Option<String> {
        let tabs = crate::lock(&self.tabs);
        let slot = tabs.get(&tab)?;
        let url = slot.kept.as_deref()?;
        Some(match slot.apart {
            true => page(&named(url), "No Preview for"),
            false => page(&place(url), "Could Not Load"),
        })
    }

    /// The script that loads tab `tab` again from inside its document, as an agent's Reload
    /// runs it: the address the tab keeps, else the page it shows. The line's own document is
    /// blank, and reloading that would ask for nothing.
    pub(super) fn reloading(&self, tab: u32) -> String {
        match self.address(tab) {
            Some(url) => format!("location.replace({})", serde_json::Value::String(url)),
            None => "location.reload()".into(),
        }
    }

    /// Tab `tab` is gone: the next one of its number starts with nothing.
    pub(super) fn forget(&self, tab: u32) {
        crate::lock(&self.tabs).remove(&tab);
    }
}

/// `view`, tab `tab`'s, loads again: the address it could not load, else the page it shows.
pub(super) fn reload(view: &wry::WebView, tab: u32, shared: &Shared) {
    let _ = match shared.unloaded.address(tab) {
        Some(url) => view.load_url(&url),
        None => view.reload(),
    };
}

/// The first pause before a waiting tab is asked whether its view still loads.
const FIRST: Duration = Duration::from_millis(120);
/// How many times it is asked: about two minutes, past the time WebKit itself gives a server.
const TIMES: u32 = 130;

/// The pause before a waiting tab is asked the `time`-th time: a dead address is known at
/// once, so the first come quickly, and a slow server is asked once a second.
fn pause(time: u32) -> Duration {
    FIRST * time.clamp(1, 8)
}

/// Tab `tab` waits for what was asked of it at `at`: until it comes, is given up or is
/// replaced, the window is asked now and then whether the tab's view still loads
/// ([`Cmd::Unanswered`]), since nothing tells it of a load given up.
pub(super) fn watch(shared: &Arc<Shared>, proxy: &EventLoopProxy<Cmd>, tab: u32, at: u64) {
    let (shared, proxy) = (Arc::clone(shared), proxy.clone());
    let _ = std::thread::Builder::new()
        .name("desk-unloaded".into())
        .spawn(move || {
            for time in 1..=TIMES {
                #[expect(
                    clippy::disallowed_methods,
                    reason = "the wait is the point: the view is asked now and then, on the \
                              window's thread, whether it still loads"
                )]
                std::thread::sleep(pause(time));
                let waits = shared.unloaded.waits(tab, at);
                if !waits || proxy.send_event(Cmd::Unanswered(tab, at)).is_err() {
                    return;
                }
            }
        });
}

/// `text` with what would be markup spelled out.
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The place `url` names, as the line says it: its host, with a port that is not the scheme's
/// own; what is no address, as written.
fn place(url: &str) -> String {
    let named = url::Url::parse(url).ok().and_then(|parsed| {
        let host = parsed.host_str()?.to_owned();
        Some(match parsed.port() {
            Some(port) => format!("{host}:{port}"),
            None => host,
        })
    });
    escaped(&named.unwrap_or_else(|| url.to_owned()))
}

/// The file `url` names, as the line says it: the last part of its path, as written before it
/// was spelled for an address; an address that names none, its place.
fn named(url: &str) -> String {
    super::guard::file_named(url).map_or_else(|| place(url), |name| escaped(&name))
}

/// The document a tab shows for what did not come, or came as nothing a page shows: one quiet
/// line (`says`, then `what`, already free of markup), centred, on the app's ground and in its
/// tokens, titled with `what` so the tab is named for it.
fn page(what: &str, says: &str) -> String {
    let css = frame::CSS;
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>{what}</title><style>{css}\
         body{{display:flex;align-items:center;justify-content:center;box-sizing:border-box;\
         padding:0 16px;text-align:center;font:13px/16px var(--ui-sys);color:var(--ink-2);\
         cursor:default;-webkit-user-select:none;user-select:none}}p{{margin:0}}</style></head>\
         <body><p role=\"status\">{says} {what}</p></body></html>"
    )
}

#[cfg(test)]
#[path = "unloaded_tests.rs"]
mod tests;
