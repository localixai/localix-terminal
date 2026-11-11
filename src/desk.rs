//! The desk: the `window` tool's actions over the window's tabs, kept per owner. An owner is a
//! project (or a name an agent chose with `--as`); it opens, acts in, pictures and closes its own
//! tabs, and never sees another's. Pages are `w1`, `w2`…; terminal sessions `t1`, `t2`…. The
//! window is the person's: they may show, close or comment on any tab.
//!
//! Here, the desk itself and how a call is answered; `open`, `interact` and the rest are
//! in `actions`, a page's tab in `web`, a terminal's in `term`, the board's in `board`.

mod actions;
mod args;
mod beside;
mod board;
mod board_file;
mod board_ops;
mod board_run;
mod board_told;
mod call;
mod center;
mod changes;
mod events;
mod heard;
mod layout;
mod layout_drawn;
mod layout_moves;
mod layout_resize;
mod ledger;
mod lifecycle;
mod open;
mod registry;
mod relay;
mod restore;
mod scope;
mod tab;
mod term;
mod told;
mod web;
mod workers;
mod writes;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::browser::{Browser, Page};
use crate::feed::{Entity, Op};
use crate::projects::{Project, ProjectId};
use crate::served::Served;
use crate::terminal::Session;

pub use args::{description, schema};
#[cfg(test)]
use call::plain;
pub use call::{Answered, Call, Failure, Output};
pub use center::agent_of;
pub use events::{Events, Quiet, said_block};
use registry::Registry;
pub use registry::Said;

pub struct Desk {
    /// The window's pages; none where no window shows (no display, or by choice).
    page: Option<Page>,
    browser: Option<Browser>,
    sessions: Mutex<HashMap<u32, Arc<Session>>>,
    /// The workers the person ran from a board, by the terminal each runs in (`workers.rs`).
    workers: Mutex<HashMap<String, workers::Worker>>,
    /// The project (the window's key) each session was started for: kept as long as the session
    /// is, whatever becomes of its tab and its row of the ledger.
    session_groups: Mutex<HashMap<u32, String>>,
    /// Held while a session starts, so two starts never both take the last place.
    starting: Mutex<()>,
    next_terminal: AtomicU32,
    registry: Mutex<Registry>,
    served: Served,
    events: Arc<dyn Events>,
    /// The one way the desk says what changed: owners record, consumers (the relay, the Event
    /// Center) read.
    feed: crate::feed::Feed,
    /// The Event Center: what the person has seen and dismissed of it.
    center: Mutex<center::Center>,
    /// Held from the Event Center's gathering to its painting, and while the person's marks are
    /// kept: a paint is never of an older gathering than the one before it.
    painting: Mutex<()>,
    /// What the agent asked the person to do, by tab, for the Event Center.
    asks: Mutex<HashMap<center::Subject, center::Ask>>,
    /// Each web tab's page as the caller last read it whole.
    seen: Mutex<HashMap<u32, Arc<Mutex<changes::Seen>>>>,
    /// One lock per web tab: work in one never waits on another's.
    locks: Mutex<HashMap<u32, Arc<Mutex<()>>>>,
    /// Opening and closing tabs, one at a time.
    tabs_change: Mutex<()>,
    shots: PathBuf,
    /// The secrets calls had put in: no answer shows their values.
    vault: crate::secrets::Vault,
    /// Each project's board, once it has been asked for.
    boards: Mutex<HashMap<ProjectId, board::Kept>>,
    next_board: AtomicU32,
    /// Where an owner's board news waits for the person to be quiet.
    board_news: Mutex<Option<std::sync::mpsc::SyncSender<ProjectId>>>,
    /// The projects called for in this run, by canonical folder: each found or made once.
    projects: Mutex<HashMap<PathBuf, Project>>,
    /// The desk itself, for what the window calls back.
    me: Weak<Self>,
    /// What the window told that is written to the store, in order, on a thread of its own:
    /// the window's loop never waits on the disk.
    writes: OnceLock<writes::Writes>,
    /// Each open tab's row in the ledger, and its project, by key.
    ledger: Mutex<HashMap<String, (crate::ledger::Row, ProjectId)>>,
    /// Which pane was opened from which terminal of the window (beside.rs).
    links: Mutex<beside::Links>,
    /// Each terminal's title writes, so a title said often is written once in a while.
    titled: Mutex<HashMap<String, ledger::Titled>>,
    /// The app is ending: its tabs go with it, yet stay open in the ledger, a split kept.
    ending: AtomicBool,
    /// The projects that have had a tab this run: what another run left open in them is past.
    begun: Mutex<std::collections::HashSet<ProjectId>>,
    /// What ends the app when the person quits it (`on_quit`).
    quit: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Desk {
    /// A desk over `browser`'s window, or with no window at all (terminals only).
    pub fn new(
        browser: Option<Browser>,
        headless: bool,
        shots: PathBuf,
        events: Arc<dyn Events>,
        store: Arc<crate::store::Store>,
    ) -> Arc<Self> {
        let desk = Arc::new_cyclic(|weak: &Weak<Self>| {
            if let Some(browser) = &browser {
                let weak = Weak::clone(weak);
                browser.listen(move |key, heard| {
                    if let Some(desk) = weak.upgrade() {
                        desk.heard(key, heard);
                    }
                });
                browser.answers(Arc::new(crate::offers::Kept(Arc::clone(&store))));
            }
            Self {
                page: browser.clone().map(|browser| Page::new(browser, headless)),
                browser,
                sessions: Mutex::default(),
                workers: Mutex::default(),
                session_groups: Mutex::default(),
                starting: Mutex::default(),
                next_terminal: AtomicU32::new(1),
                registry: Mutex::default(),
                served: Served::default(),
                events,
                feed: crate::feed::Feed::new(store, std::time::Duration::from_millis(500)),
                center: Mutex::default(),
                painting: Mutex::new(()),
                asks: Mutex::default(),
                seen: Mutex::default(),
                locks: Mutex::default(),
                tabs_change: Mutex::new(()),
                shots,
                vault: crate::secrets::Vault::default(),
                boards: Mutex::default(),
                next_board: AtomicU32::new(1),
                board_news: Mutex::default(),
                projects: Mutex::default(),
                me: Weak::clone(weak),
                writes: OnceLock::new(),
                ledger: Mutex::default(),
                links: Mutex::default(),
                titled: Mutex::default(),
                ending: AtomicBool::new(false),
                begun: Mutex::default(),
                quit: OnceLock::new(),
            }
        });
        // A picture someone asked for earlier is not worth keeping for long.
        crate::shots::prune(&desk.shots);
        let relay = relay::Relay {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(relay)) {
            log::warn!("desk: nobody will be told what changes: {error}");
        }
        desk.settle_last_run();
        let center = center::Projector {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(center)) {
            log::warn!("desk: the Event Center will not follow what changes: {error}");
        }
        desk.gather_windows();
        let drawn = layout_drawn::Drawn {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(drawn)) {
            log::warn!("desk: the window will not follow the layout: {error}");
        }
        let files = board_file::BoardFile {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(files)) {
            log::warn!("desk: the boards will not be written out to their projects: {error}");
        }
        desk
    }

    /// The shells of the terminals to come keep what they read at their start in `folder`.
    pub fn keep_shell_files_in(&self, folder: PathBuf) {
        crate::terminal::keep_shell_files_in(folder);
    }

    /// What the person sent from the window to `owner`'s tabs since it was last asked, taken.
    pub fn said(&self, owner: &str) -> Option<String> {
        let said = crate::lock(&self.registry).take_said(owner);
        let notes: Vec<String> = said.into_iter().map(|said| said.text).collect();
        (!notes.is_empty()).then(|| said_block(&notes))
    }

    /// What the person sent to `owner` to act on now that was not delivered yet, left as it
    /// is: told again to whoever subscribes, until acknowledged. Quiet news waits for a call.
    pub fn pending(&self, owner: &str) -> Vec<Said> {
        crate::lock(&self.registry).pending(owner)
    }

    /// `owner` has the notes `ids` name.
    pub fn acknowledge(&self, owner: &str, ids: &[u64]) {
        crate::lock(&self.registry).acknowledge(owner, ids);
    }

    /// Kept `words` for the owner of tab `key`; with `told`, the notice that tells those who
    /// subscribed, recorded under the registry's lock with the note it announces.
    fn hear(&self, key: &str, words: &str, told: bool) {
        self.hear_for(key, None, words, told);
    }

    /// `hear`, for `to` when the words are for another agent of the tab's project.
    fn hear_for(&self, key: &str, to: Option<&str>, words: &str, told: bool) {
        let mut registry = crate::lock(&self.registry);
        if let Some((owner, said)) = registry.hear(key, to, words, told)
            && told
        {
            let said = Entity::Said { owner, id: said.id };
            self.feed.record(&said, Op::Upserted);
        }
    }

    /// Tab `key`, whose was `owner`, is gone: closed in the ledger too.
    fn gone(&self, owner: Option<String>, key: &str) {
        self.unkeep(key);
        let key = key.to_string();
        self.feed.record(&Entity::Pane { owner, key }, Op::Deleted);
    }

    /// The person does `action` on project `project`'s board, as its page would send it: for
    /// tests, which have no window to do it in.
    #[cfg(feature = "test-server")]
    pub fn person_on_the_board(&self, project: ProjectId, action: &str) {
        self.person_on_board(project, action);
    }

    /// The person sends `words` about tab `key`, as they would from the window: for tests, which
    /// have no window to send from.
    #[cfg(feature = "test-server")]
    pub fn person_says(&self, key: &str, words: &str) {
        self.kept(key, words);
    }

    /// `words` kept for the owner of tab `key`, and told to those who subscribed.
    fn kept(&self, key: &str, words: &str) {
        self.hear(key, words, true);
    }

    /// The pages the window no longer has (the person closed it, or the tab) forgotten as a
    /// closed tab is: a call on one is told there is no such tab, `list` names none, and the
    /// owner hears `tab/closed`.
    fn prune_pages(&self) {
        let Some(page) = &self.page else { return };
        let alive: Vec<String> = match &self.browser {
            Some(browser) if page.is_open() => browser.pages(),
            Some(_) => vec![],
            None => return,
        };
        let gone: Vec<(String, Option<String>)> = {
            let mut registry = crate::lock(&self.registry);
            let pages = registry.pages();
            (pages.into_iter())
                .filter(|key| !alive.contains(key))
                .map(|key| {
                    let owner = registry.owner_named(&key);
                    registry.release(&key);
                    (key, owner)
                })
                .collect()
        };
        for (key, owner) in gone {
            if let Some(id) = key.get(1..).and_then(|n| n.parse::<u32>().ok()) {
                self.forget_page(id, 1);
            }
            self.gone(owner, &key);
        }
    }

    fn owner_of(&self, key: &str) -> Option<String> {
        crate::lock(&self.registry).owner_named(key)
    }

    /// Close every tab `owner` opened (the person's own, in the same project, stay); how many.
    pub fn end(&self, owner: &str) -> usize {
        let tabs = crate::lock(&self.registry).tabs_opened_by(owner);
        for tab in &tabs {
            let _ = self.close_tab(tab);
        }
        crate::lock(&self.registry).forget(owner);
        // What the session asked the person to do goes with it.
        crate::lock(&self.asks).remove(&center::Subject::Session(owner.to_string()));
        tabs.len()
    }

    /// How long the app's quit waits for the writes the window told last.
    const LAST_WRITES: std::time::Duration = std::time::Duration::from_secs(5);

    /// Every terminal ends; the window stays the process's to close.
    pub fn shutdown(&self) {
        // The tabs open now stay open in the ledger, their screens kept: the app opens them
        // again. Their sessions end below, and must not close them, nor the split they make.
        self.ending.store(true, std::sync::atomic::Ordering::SeqCst);
        self.keep_screens();
        crate::lock(&self.ledger).clear();
        crate::lock(&self.session_groups).clear();
        // What the window told last is written before the feed stops taking notices.
        if !self.writes_done(Self::LAST_WRITES) {
            log::warn!("desk: the last writes were not done as the app quit");
        }
        self.feed.store().optimize();
        self.feed.stop();
        let sessions = std::mem::take(&mut *crate::lock(&self.sessions));
        sessions.values().for_each(|session| session.close());
    }

    /// Whether a board or a terminal is in the window: it shows the window as much as a page does.
    pub(super) fn panels_open(&self) -> bool {
        !crate::lock(&self.sessions).is_empty()
            || (crate::lock(&self.boards).values()).any(|kept| kept.panel.is_some())
    }

    /// The window and every owner's tabs, for `llx-desk status`.
    pub fn status(&self) -> String {
        let window = match (&self.page, &self.browser) {
            (Some(page), _) if page.is_open() || self.panels_open() => "open",
            // Shown with no tab in it: the window's start of a new tab.
            (Some(_), Some(browser)) if browser.shown() => "open, no tab",
            (Some(_), _) => "not open yet",
            (None, _) => "none (no display, or --no-window)",
        };
        let registry = crate::lock(&self.registry);
        let mut out = format!(
            "window: {window}\nterminals: {}",
            crate::lock(&self.sessions).len()
        );
        if let Some(why) = self.feed.store().kept_in_memory() {
            let _ = write!(
                out,
                "\nstorage: memory only, nothing is kept (database: {why})"
            );
        }
        for (owner, tabs) in registry.owners() {
            let _ = write!(out, "\n{owner}: {}", tabs.join(" "));
        }
        out
    }

    /// The browser tab `id`'s lock.
    fn tab_lock(&self, id: u32) -> Arc<Mutex<()>> {
        Arc::clone(crate::lock(&self.locks).entry(id).or_default())
    }
}

#[cfg(test)]
#[path = "desk_tests.rs"]
mod tests;
