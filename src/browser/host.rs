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

use crate::WindowTab;

use super::capture::{Clip, Shot};
use super::comments::Comments;
use super::panel;
use super::strip::{Click, Key};
pub use super::told::{
    Answers, Arranged, Arranging, Asked, CenterDid, Confirm, Halves, Heard, Ledger, Past, Place,
    Tab, TabKind, Visited, Went,
};
use super::viewport::Viewport;

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

pub(super) enum Cmd {
    /// Open the window (once); the reply says whether it opened.
    Open {
        window: Window,
        reply: SyncSender<Result<(), String>>,
    },
    /// A script built by the driver that hands a probe call or its promise to the probe; `false`
    /// or a parse error answers the call at once with `failed` (`probe_not_ready`,
    /// `script_not_started`).
    Eval {
        id: u64,
        script: String,
        failed: &'static str,
        /// In this panel's page instead, answered with the script's value (for checks).
        panel: Option<u32>,
        /// In this browser tab instead of the one acted on.
        tab: Option<u32>,
    },
    /// Load an address: in this tab, else the one acted on.
    Navigate(String, Option<u32>),
    /// Open a tab on `url` and show it. A page's own new window (`target=_blank`, `window.open`)
    /// comes here with no reply.
    NewTab {
        url: String,
        reply: Option<SyncSender<Result<u32, String>>>,
        /// The tab whose page asked for it, whose project it is of.
        opener: Option<u32>,
    },
    /// A new tab's page has drawn: the tab draws its own ground again (until then the window's
    /// showed through it, so a blank view never flashes white over a dark window).
    Reveal(u32),
    SwitchTab {
        id: u32,
        reply: SyncSender<Result<(), String>>,
    },
    /// Close a tab; the reply says whether tabs are left (with none, the window closed).
    CloseTab {
        id: u32,
        reply: SyncSender<Result<bool, String>>,
    },
    /// Close the window, as the person would but without stopping the task: it opens again,
    /// shown or hidden, on its next use.
    Close,
    /// Give a tab (the one acted on with none) a size of its own (sizes.rs), or the whole window.
    Frame(Option<u32>, Option<super::sizes::Frame>),
    /// A screenshot of the page, or of a part of it; the reply comes once WebKit has drawn it.
    Capture {
        clip: Option<Clip>,
        reply: SyncSender<Result<Shot, String>>,
        tab: Option<u32>,
    },
    /// Open a panel as a tab: its id, title and page, and what it shares with its caller.
    Panel(panel::Opening),
    /// Run a panel's newest frame.
    Paint(u32),
    /// Close a panel.
    Unpanel(u32),
    /// A panel's menu, where the person right-clicked in it (`m` of its page).
    PanelMenu(u32, (u32, u32), Vec<super::choice::Entry>),
    /// A panel's page took a title.
    Titled(u32, String),
    /// What the person clicked on the strip.
    Clicked(String),
    /// What the person clicked in the side: as the strip says it, its points the side's.
    SideClicked(String),
    /// The person clicked in the browser's toolbar: said as the strip says it, its points the
    /// toolbar's.
    NavClicked(String),
    /// The person's settings changed: the window takes them.
    Restyle,
    /// A comment the person made on a page: pictured, then held.
    Noted(Noted),
    /// Tell the strip what the window holds (a tab's title changed, its page loaded).
    Strip,
    /// Integration `.0` was read: on (`.1`) or off. The offer bar is drawn again.
    Offered(&'static str, bool),
    /// The window goes to the project of this key (one the app just made, the person's pick).
    Go(String),
    /// The layout of the windows' projects (`Browser::arranged`), and the windows open, the main
    /// one first: drawn as it is.
    Arranged(Vec<i64>, Vec<Arranged>),
    /// The app opened window `i64` (`Browser::new_window`): the window in use from now on, its
    /// deck made with the first tab it is given.
    NewWindow(i64),
    /// Take in the popups pages opened (popup.rs), and show the last.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            dead_code,
            reason = "elsewhere a page's window opens as a tab of its own"
        )
    )]
    Adopt,
    /// For checks: how the window is dressed, once tinted again.
    Dressed(SyncSender<Option<Dressing>>),
    /// The window's tabs; `None` while it is closed.
    Listing(SyncSender<Option<Vec<WindowTab>>>),
    /// Show or close a tab by its key, for the agent: the task goes on whatever closes.
    Act(Click, SyncSender<Result<(), String>>),
    /// What the agent or the person did to the window's presence (presence.rs).
    Presence(super::presence::Event),
    /// The person pressed in one of the app's windows (an NSWindow's address), at `x`, `y`
    /// points from its top left (presses.rs).
    Pressed(usize, f64, f64),
    /// The person clicked the mini open: expanded, and the app they use from then on.
    Opened,
    /// The mini's picture, taken or not.
    Pictured(Result<Shot, String>),
    /// For checks: the window's state, `hidden`, `mini` or `expanded`.
    Presented(SyncSender<&'static str>),
    /// Ask the person; the reply is whether they went on.
    Confirm(Confirm, SyncSender<bool>),
    Exit(i32),
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

/// A comment the person made on browser tab `tab`, in words, and the part of the view it is about.
pub(super) struct Noted {
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

    /// The window was closed: every call waiting on it fails now, rather than at its timeout, and
    /// the load counts start over, so a window opened again waits for its own first document.
    pub(super) fn closed(&self) {
        self.open.store(false, Ordering::SeqCst);
        let waiting = std::mem::take(&mut *crate::lock(&self.pending));
        for sender in waiting.into_values() {
            let _ = sender.send(Err(WINDOW_CLOSED.into()));
        }
        crate::lock(&self.loads).clear();
        crate::lock(&self.tabs).clear();
        crate::lock(&self.pages).clear();
        self.notices.forget_all();
        // No window, no one commenting: actions waiting for the person go (and find it closed).
        self.turn.commenting(false);
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

    pub(super) fn titled(&self, tab: u32, title: &str) {
        let url = (crate::lock(&self.tabs).iter_mut().find(|t| t.id == tab)).map(|entry| {
            title.clone_into(&mut entry.title);
            entry.url.clone()
        });
        if let Some(url) = url.filter(|url| !url.is_empty()) {
            self.went(tab, &url, title);
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

    /// A message from the page: `{t, id, ok}` or `{t, id, err}`, or `{t, fb}`, a comment of the
    /// person's on the tab shown, given back to be pictured and held. Anything else is ignored.
    pub(super) fn ipc(&self, body: &str) -> Option<Said> {
        let message = serde_json::from_str::<Value>(body).ok()?;
        if message.get("t").and_then(Value::as_str) != Some(self.token.as_str()) {
            return None;
        }
        if let Some(url) = message.get("close").and_then(Value::as_str) {
            // The newest tab at that address: the popup, not its opener (never the first tab).
            let tabs = crate::lock(&self.tabs);
            let tab = tabs.iter().rev().find(|tab| tab.id != 1 && tab.url == url);
            return tab.map(|tab| Said::Close(tab.id));
        }
        if let Some(fb) = message.get("fb") {
            let tab = self.active.load(Ordering::SeqCst);
            return super::feedback::said(fb).map(|said| {
                Said::Noted(Noted {
                    tab,
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
