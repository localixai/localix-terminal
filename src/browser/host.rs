//! The native window: a `tao` event loop on the main thread that opens one window when asked (it
//! exists only once it is needed): the browser's tabs and the terminal sessions' screens are its
//! tabs (deck.rs), with a `wry` WebView each. The caller's work runs on a worker thread and
//! reaches the page through [`Cmd`]s on the event loop's proxy; answers come back over the page's
//! IPC channel, matched by call id and a per-run token the page cannot read. Here, what they
//! share; the loop is host_events.rs (the browser's commands host_web.rs), the work thread's
//! handle host_handle.rs.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::event_loop::EventLoopProxy;
use wry::PageLoadEvent;

use super::capture::{Clip, Shot};
use super::comments::Comments;
use super::panel;
use super::strip::Key;
pub use super::told::{
    Answers, Arranged, Arranging, Asked, CenterDid, Confirm, Halves, Heard, Ledger, Past, Place,
    Tab, TabKind, Visited, Went,
};
use super::viewport::Viewport;

#[path = "host_commands.rs"]
mod commands;
pub(super) use commands::Cmd;

#[path = "host_events.rs"]
mod events;
#[path = "host_handle.rs"]
mod handle;
#[path = "host_presence.rs"]
mod presence;
#[path = "host_web.rs"]
mod web;
#[path = "host_window.rs"]
mod window;
pub use events::host;

/// One script from eleven files: snapshot, the page's requests, what the page tells (dialogs,
/// announcements, refused fields), same-origin frames, the outline, refs found again after a
/// render, actions, typing, keys, the person's comments, then evaluate and the IPC entry point.
pub(super) const PROBE: &str = concat!(
    include_str!("probe/snapshot.js"),
    include_str!("probe/network.js"),
    include_str!("probe/told.js"),
    include_str!("probe/frames.js"),
    include_str!("probe/outline.js"),
    include_str!("probe/refind.js"),
    include_str!("probe/act.js"),
    include_str!("probe/typing.js"),
    include_str!("probe/keys.js"),
    include_str!("probe/feedback.js"),
    include_str!("probe/evaluate.js"),
);

/// How the window is opened.
#[derive(Debug, Clone)]
pub struct Window {
    pub url: String,
    pub title: String,
    /// Keep the window off screen and out of the Dock; the page still renders.
    pub headless: bool,
    /// The page's size.
    pub viewport: Viewport,
}

pub(super) type Reply = Result<Value, String>;

/// How a window is dressed, for checks: see [`Browser::dressed`].
pub type Dressing = (usize, isize, bool, f64, bool);

/// State the event loop and the worker share.
pub(super) struct Shared {
    pub(super) token: String,
    /// Whether the window is open now: the person may close it, and it opens again when needed.
    pub(super) open: AtomicBool,
    pub(super) next_id: AtomicU64,
    pub(super) pending: Mutex<HashMap<u64, SyncSender<Reply>>>,
    /// Each tab's loads, by tab id.
    pub(super) loads: Mutex<HashMap<u32, Loads>>,
    pub(super) loaded: Condvar,
    /// The tab acted on.
    pub(super) active: AtomicU32,
    /// Numbers every load event across the tabs, so a load in another tab never reads as this
    /// tab's (a page that opened a tab has started a load: the new tab's).
    seq: AtomicU64,
    pub(super) tabs: Mutex<Vec<Tab>>,
    /// Each window's pages open now, by the native window: `tabs` is all of them.
    pub(super) pages: Mutex<HashMap<tao::window::WindowId, Vec<u32>>>,
    /// Who hears what the person does in the window.
    listener: Mutex<Option<Listener>>,
    /// The group (a project) of each tab, by key, as its opener named it.
    pub(super) groups: Mutex<HashMap<String, String>>,
    /// The projects the window knows, as the app tells them (`Browser::places`).
    pub(super) places: Mutex<Vec<Place>>,
    /// Which project the window shows, and what it knows of the others (host_project.rs).
    pub(super) scope: Mutex<super::scope::Scope>,
    /// The window's own Settings tab, once opened.
    pub(super) settings: Mutex<Option<panel::Panel>>,
    /// What a terminal's program says it is doing (the title it set), by key.
    pub(super) details: Mutex<HashMap<String, String>>,
    /// The new tabs drawn through until their page has drawn (tabs::see_through_until_drawn).
    pub(super) undrawn: Mutex<HashSet<u32>>,
    /// The Event Center's events, as the desk last sent them (a JSON list, newest first).
    pub(super) center: Mutex<String>,
    /// Each project's past (its tabs, its pages), as the app tells it (`Browser::ledger`), by
    /// the project's key: for the history menu and the address field's suggestions.
    pub(super) ledgers: Mutex<HashMap<String, Ledger>>,
    /// Each window's projects' tabs as the layout keeps them (`Cmd::Arranged`), by the window and
    /// the project's key: their order and their splits. Written only as the app tells it: the
    /// window keeps none of its own.
    pub(super) arranged: Mutex<HashMap<(i64, String), Arranged>>,
    /// The window in use, by its row in the layout (0: not known yet): the one the person works
    /// in, where what opens with no window named goes.
    pub(super) in_use: std::sync::atomic::AtomicI64,
    /// The person's comments on the pages, until they send them.
    pub(super) comments: Comments,
    /// Whose turn it is: the agent's actions wait while the person comments.
    pub(super) turn: super::turn::Turn,
    /// What each tab's pages told besides their content, and how it answers a confirm.
    pub(super) notices: super::notices::Notices,
    /// Where the person put the window and its mini, for the process.
    pub(super) kept: Mutex<super::placement::Kept>,
    /// What the offer bar knows: the agents in the terminals, the integrations' states.
    pub(super) offers: super::offer::Offers,
    /// Where each terminal is now (its program's folder), by tab key: its menu works there.
    pub(super) folders: Mutex<HashMap<String, std::path::PathBuf>>,
}

/// Told what the person did to a tab (its key; `window` for the whole window), in words.
type Listener = Arc<dyn Fn(&str, Heard<'_>) + Send + Sync>;

/// A comment the person made on a browser tab, in words, and the part of the view it is about.
pub(super) struct Noted {
    /// The tab it is on: 0 as the page says it (it is no one's word); the window the person
    /// comments in puts its own shown tab here (host_web.rs), never the app's last one.
    pub tab: u32,
    pub said: String,
    pub clip: Option<Clip>,
}

/// What a page said of its own accord.
pub(super) enum Said {
    /// The person's comment.
    Noted(Noted),
    /// A popup closed itself: its tab's id.
    Close(u32),
}

/// The most characters of a page's title kept: what a page sets is the strip's, the side's and the
/// ledger's text, so a page cannot make them any larger.
const TITLE: usize = 300;

/// `title` cut to [`TITLE`] characters.
fn clipped(title: &str) -> String {
    title.chars().take(TITLE).collect()
}

/// The loads of tab `active`.
pub(super) fn active_loads(loads: &HashMap<u32, Loads>, active: u32) -> Loads {
    loads.get(&active).copied().unwrap_or_default()
}

/// Page loads started and finished; equal counts mean nothing is loading. `requested` counts the
/// navigations the page asked for: WebKit keeps the old document until a new one commits, so an
/// action is only settled once a navigation it requested has loaded.
#[derive(Default, Clone, Copy)]
pub(super) struct Loads {
    pub requested: u64,
    pub started: u64,
    pub finished: u64,
}

impl Shared {
    /// Tell the listener what the person did to `key`.
    pub(super) fn heard(&self, key: &str, what: Heard<'_>) {
        // Cloned out: the app's listener runs with no guard held.
        let listen = crate::lock(&self.listener).clone();
        if let Some(listen) = listen {
            listen(key, what);
        }
    }

    fn answer(&self, id: u64, reply: Reply) {
        let waiting = crate::lock(&self.pending).remove(&id);
        if let Some(sender) = waiting {
            let _ = sender.send(reply);
        }
    }

    /// Window `window`'s browser closed, its pages `ids` with it: what is kept of them goes, and
    /// no other window's pages, loads or notices do. With no page left in any window, every call
    /// waiting fails now, rather than at its timeout, and no one is commenting; a window opened
    /// again then waits for its own first document. (A call waiting on a page of this window while
    /// others remain ends at its timeout, as when a tab closes.)
    pub(super) fn closed(&self, window: tao::window::WindowId, ids: &[u32]) {
        let left: Vec<u32> = {
            let mut pages = crate::lock(&self.pages);
            pages.remove(&window);
            pages.values().flatten().copied().collect()
        };
        crate::lock(&self.tabs).retain(|tab| left.contains(&tab.id));
        {
            let mut loads = crate::lock(&self.loads);
            for id in ids {
                loads.remove(id);
            }
        }
        for id in ids {
            self.notices.forget(*id);
        }
        self.open.store(!left.is_empty(), Ordering::SeqCst);
        if left.is_empty() {
            let waiting = std::mem::take(&mut *crate::lock(&self.pending));
            for sender in waiting.into_values() {
                let _ = sender.send(Err(WINDOW_CLOSED.into()));
            }
            // No page, no one commenting: actions waiting for the person go (and find it closed).
            self.turn.commenting(false);
        }
        self.loaded.notify_all();
    }

    pub(super) fn navigation_requested(&self, tab: u32) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        crate::lock(&self.loads).entry(tab).or_default().requested = next;
        self.loaded.notify_all();
    }

    pub(super) fn page_load(&self, tab: u32, event: &PageLoadEvent, url: &str) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut loads = crate::lock(&self.loads);
            let loads = loads.entry(tab).or_default();
            match event {
                PageLoadEvent::Started => loads.started = next,
                PageLoadEvent::Finished => loads.finished = loads.started,
            }
        }
        let title = (crate::lock(&self.tabs).iter_mut().find(|t| t.id == tab)).map(|entry| {
            entry.url = url.to_string();
            entry.title.clone()
        });
        if matches!(event, PageLoadEvent::Finished) {
            self.went(tab, url, &title.unwrap_or_default());
        }
        self.loaded.notify_all();
    }

    /// Tab `tab`'s page took `title`, clipped (a page sets it as often and as long as it likes):
    /// the app hears of it when it is not the title the tab had.
    pub(super) fn titled(&self, tab: u32, title: &str) {
        let title = clipped(title);
        let url = (crate::lock(&self.tabs).iter_mut().find(|t| t.id == tab)).and_then(|entry| {
            if entry.title == title {
                return None;
            }
            entry.title.clone_from(&title);
            Some(entry.url.clone())
        });
        if let Some(url) = url.filter(|url| !url.is_empty()) {
            self.went(tab, &url, &title);
        }
    }

    /// Browser tab `tab` showed `url`, titled `title`: the app hears it, for the ledger.
    fn went(&self, tab: u32, url: &str, title: &str) {
        let key = Key::Web(tab).name();
        let group = crate::lock(&self.groups)
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let went = Went {
            url,
            title: title.trim(),
            group: &group,
        };
        self.heard(&key, Heard::Went(went));
    }

    /// Project `group`'s tabs in the window in use, as the layout keeps them.
    pub(super) fn arranged_of(&self, group: &str) -> Option<Arranged> {
        let window = self.in_use.load(Ordering::SeqCst);
        (crate::lock(&self.arranged).get(&(window, group.to_string()))).cloned()
    }

    /// Every project's tabs in the window in use, by the project's key.
    pub(super) fn arranged_here(&self) -> HashMap<String, Arranged> {
        let window = self.in_use.load(Ordering::SeqCst);
        (crate::lock(&self.arranged).iter())
            .filter(|((of, _), _)| *of == window)
            .map(|((_, group), arranged)| (group.clone(), arranged.clone()))
            .collect()
    }

    /// Window `window`'s pages open now (`ids`, every other window's as they were), `active`
    /// acted on; what is known of each is kept.
    pub(super) fn set_tabs(&self, window: tao::window::WindowId, active: u32, ids: &[u32]) {
        let all: Vec<u32> = {
            let mut pages = crate::lock(&self.pages);
            if ids.is_empty() {
                pages.remove(&window);
            } else {
                pages.insert(window, ids.to_vec());
            }
            pages.values().flatten().copied().collect()
        };
        let mut tabs = crate::lock(&self.tabs);
        tabs.retain(|tab| all.contains(&tab.id));
        for id in ids {
            if !tabs.iter().any(|tab| tab.id == *id) {
                tabs.push(Tab {
                    id: *id,
                    url: String::new(),
                    title: String::new(),
                });
            }
        }
        tabs.sort_by_key(|tab| tab.id);
        drop(tabs);
        self.active.store(active, Ordering::SeqCst);
        self.loaded.notify_all();
    }

    /// A message from the page: `{id, ok}` or `{id, err}`, or `{fb}`, a comment of the person's,
    /// given back to be pictured and held (the tab it is on is the shown one of the window they
    /// comment in). Anything else is ignored.
    pub(super) fn ipc(&self, body: &str) -> Option<Said> {
        let message = unsealed(&self.token, body)?;
        if let Some(url) = message.get("close").and_then(Value::as_str) {
            // The newest tab at that address: the popup, not its opener (never the first tab).
            let tabs = crate::lock(&self.tabs);
            let tab = tabs.iter().rev().find(|tab| tab.id != 1 && tab.url == url);
            return tab.map(|tab| Said::Close(tab.id));
        }
        if let Some(fb) = message.get("fb") {
            return super::feedback::said(fb).map(|said| {
                Said::Noted(Noted {
                    tab: 0,
                    said,
                    clip: super::feedback::framed(fb),
                })
            });
        }
        let id = message.get("id").and_then(Value::as_u64)?;
        // Held for the call only while it waits: one that gave up never files it. The check and
        // the holding are one step under the lock the call takes to stop waiting, so nothing is
        // held after the call filed what it had.
        if let Some(told) = message.get("told") {
            let pending = crate::lock(&self.pending);
            if pending.contains_key(&id) {
                self.notices.arrived(id, told);
            }
        }
        let reply = match message.get("err").and_then(Value::as_str) {
            Some(error) => Err(error.to_string()),
            None => Ok(message.get("ok").cloned().unwrap_or(Value::Null)),
        };
        self.answer(id, reply);
        None
    }
}

/// What the probe sent: the run's token, a line break, then the message as JSON. The token is
/// plain text in front of the JSON, never a member of it, so no `toJSON` of the page's can read
/// it from the object that is stringified. None for anything else, or for JSON that does not parse.
fn unsealed(token: &str, body: &str) -> Option<Value> {
    let rest = body.strip_prefix(token)?.strip_prefix('\n')?;
    let parsed = serde_json::from_str(rest);
    if let Err(error) = &parsed {
        super::debug(|| format!("a page message did not parse: {error}"));
    }
    parsed.ok()
}

/// The browser host as the work thread holds it: it opens the one window on demand.
#[derive(Clone)]
pub struct Browser {
    proxy: EventLoopProxy<Cmd>,
    shared: Arc<Shared>,
}

/// The panel id that names the strip's own page, for checks.
pub(super) const STRIP: u32 = u32::MAX;

/// The panel id that names the mini's own page, for checks.
pub(super) const MINI: u32 = u32::MAX - 1;

/// The panel id that names the page the tabs are listed in, for checks: the side, or the strip
/// where there is none.
pub(super) const TABS: u32 = u32::MAX - 2;
/// The browser's toolbar's page, for checks.
pub(super) const NAV_PAGE: u32 = u32::MAX - 3;

/// The answer to every call while the window is closed.
pub(super) const WINDOW_CLOSED: &str = "window_closed";

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
