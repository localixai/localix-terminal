//! The desk: the `window` tool's actions over the window's tabs, kept per owner. An owner is a
//! project (or a name an agent chose with `--as`); it opens, acts in, pictures and closes its own
//! tabs, and never sees another's. Pages are `w1`, `w2`…; terminal sessions `t1`, `t2`…. The
//! window is the person's: they may show, close or comment on any tab.
//!
//! Here, the desk itself and how a call is answered; `open`, `interact` and the rest are
//! in `actions`, a page's tab in `web`, a terminal's in `term`, the board's in `board`.

mod actions;
mod args;
mod board;
mod board_file;
mod board_told;
mod center;
mod changes;
mod events;
mod ledger;
mod lifecycle;
mod open;
mod registry;
mod relay;
mod scope;
mod splits;
mod term;
mod told;
mod web;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use serde_json::{Value, json};

use crate::browser::{Asked, Browser, HOME, Heard, Page, Shot};
use crate::feed::{Entity, Op};
use crate::projects::{Project, ProjectId};
use crate::served::Served;
use crate::terminal::Session;

pub use args::{ask_field, board_fields, description, schema};
pub use events::{Events, Quiet, said_block};
use registry::Registry;
pub use registry::Said;

/// Who is calling, from where, and with what the call may need of the caller.
#[derive(Clone, Copy)]
pub struct Call<'a> {
    /// The caller's key (`projects::owner`): its tabs and notes are kept by it.
    pub owner: &'a str,
    /// The project it works in: everything it opens is the project's.
    pub project: &'a Project,
    /// Its name within the project; none: the project's own agent.
    pub agent: Option<&'a str>,
    /// The caller's directory: relative paths start here.
    pub cwd: &'a Path,
    /// The caller's environment, for a program started in a terminal.
    pub vars: Option<&'a [(String, String)]>,
    /// A look for a pane, not the agent's call: the person's notes stay for the agent.
    pub peek: bool,
}

/// Why a call did not come to what it asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// This call failed; the next may not.
    Failed(String),
    /// Nothing like it will work until something outside changes (a key, a network).
    Fatal(String),
}

impl Failure {
    pub fn message(&self) -> &str {
        match self {
            Self::Failed(why) | Self::Fatal(why) => why,
        }
    }
}

impl From<String> for Failure {
    fn from(why: String) -> Self {
        Self::Failed(why)
    }
}

/// What a call gives back: what happened, the tab as it is now, and maybe a picture of it.
#[derive(Default)]
pub struct Output {
    pub report: String,
    pub state: Option<String>,
    pub shot: Option<Shot>,
}

impl From<String> for Output {
    fn from(report: String) -> Self {
        Self {
            report,
            ..Self::default()
        }
    }
}

/// A call's answer before it is put into words: the picture is bytes still.
pub struct Answered {
    /// The report and the tab as it is now.
    pub text: String,
    /// What the person sent from the window, as the agent reads it.
    pub said: Option<String>,
    pub shot: Option<Shot>,
}

impl Answered {
    /// The text, a line about the picture if there is one, and what the person sent.
    pub fn text_with(&self, picture: Option<&str>) -> String {
        let parts = [Some(self.text.as_str()), picture, self.said.as_deref()];
        let kept: Vec<&str> = parts
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect();
        kept.join("\n\n").trim().to_string()
    }
}

pub struct Desk {
    /// The window's pages; none where no window shows (no display, or by choice).
    page: Option<Page>,
    browser: Option<Browser>,
    sessions: Mutex<HashMap<u32, Arc<Session>>>,
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
    /// What the agent asked the person to do, by tab, for the Event Center.
    asks: Mutex<HashMap<center::Subject, center::Ask>>,
    /// Each web tab's page as the caller last read it whole.
    seen: Mutex<HashMap<u32, changes::Seen>>,
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
    /// Each open tab's row in the ledger, and its project, by key.
    ledger: Mutex<HashMap<String, (crate::ledger::Row, ProjectId)>>,
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
                starting: Mutex::default(),
                next_terminal: AtomicU32::new(1),
                registry: Mutex::default(),
                served: Served::default(),
                events,
                feed: crate::feed::Feed::new(store, std::time::Duration::from_millis(500)),
                center: Mutex::default(),
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
                ledger: Mutex::default(),
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
        let center = center::Projector {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(center)) {
            log::warn!("desk: the Event Center will not follow what changes: {error}");
        }
        let files = board_file::BoardFile {
            desk: Arc::downgrade(&desk),
        };
        if let Err(error) = desk.feed.consume(Box::new(files)) {
            log::warn!("desk: the boards will not be written out to their projects: {error}");
        }
        desk
    }

    /// One `window` call, answered as text: the report, the tab, where a picture was saved, and
    /// what the person sent.
    pub fn window(&self, call: &Call, args: &Value) -> Result<String, Failure> {
        let answered = self.call(call, args)?;
        let picture = match &answered.shot {
            Some(shot) => {
                let path = crate::shots::save(&self.shots, &shot.jpeg).map_err(Failure::Failed)?;
                Some(format!(
                    "Screenshot saved ({}x{}): {}\nRead that file to see it.",
                    shot.width,
                    shot.height,
                    path.display()
                ))
            }
            None => None,
        };
        Ok(answered.text_with(picture.as_deref()))
    }

    /// One `window` call, answered with its picture as it is, for whoever carries bytes.
    pub fn call(&self, call: &Call, args: &Value) -> Result<Answered, Failure> {
        crate::lock(&self.registry).known(call);
        let mut out = self.act(call, args)?;
        let mut text = out.report.clone();
        if let Some(state) = out.state.take() {
            text = format!("{text}\n\n{state}");
        }
        Ok(Answered {
            text: self.vault.mask(text.trim()),
            said: (!call.peek)
                .then(|| self.said(call.owner))
                .flatten()
                .map(|said| self.vault.mask(&said)),
            shot: out.shot,
        })
    }

    /// The person opened the app (app.rs): the window comes to them, expanded, as their click
    /// on the mini opens it; with no tab in it yet, their shell in `call.cwd` (their home), a
    /// terminal of their own.
    pub fn present(&self, call: &Call, go: bool) -> Result<String, String> {
        let browser = (self.browser.as_ref()).ok_or("there is no window here: no display")?;
        crate::lock(&self.registry).known(call);
        let group = crate::projects::key(call.project.id);
        let listed = browser.listing().unwrap_or_default();
        // Going to the project: the window goes first, so what opens for it opens in sight.
        let empty = match go {
            true => {
                browser.go(&group);
                !listed.iter().any(|tab| tab.group == group)
            }
            false => listed.is_empty(),
        };
        if empty {
            self.reopen_project(call, &group)?;
        }
        browser.present();
        Ok(if empty {
            "The window opened, with what the project had open.".into()
        } else {
            "The window opened.".into()
        })
    }

    /// What the person sent from the window to `owner`'s tabs since it was last asked, taken.
    pub fn said(&self, owner: &str) -> Option<String> {
        let said = crate::lock(&self.registry).take_said(owner);
        let notes: Vec<String> = said.into_iter().map(|said| said.text).collect();
        (!notes.is_empty()).then(|| said_block(&notes))
    }

    /// What the person sent to `owner` that was not delivered yet, left as it is: told again to
    /// whoever subscribes, until acknowledged.
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
        let mut registry = crate::lock(&self.registry);
        if let Some((owner, said)) = registry.hear(key, words)
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
        self.feed.record(&Entity::Tab { owner, key }, Op::Deleted);
    }

    /// What the person did in the window: a comment, a closed tab, a terminal asked for.
    fn heard(self: &Arc<Self>, key: &str, heard: Heard<'_>) {
        match heard {
            Heard::Sent(words) | Heard::Did(words) => self.kept(key, words),
            // A tab the person closed is no news worth waking the agent for: it hears it with its
            // next call. A board is kept; a page or a terminal is gone.
            Heard::Closed(words) => {
                self.hear(key, words, false);
                match key.starts_with('b') {
                    true => self.unkeep(key),
                    false => self.gone(self.owner_of(key), key),
                }
            }
            Heard::Went(went) => self.went(key, &went),
            Heard::Folder(folder) => self.open_folder(folder),
            Heard::Showing => self.showing(key),
            Heard::Split(halves) => self.split_told(key, halves),
            Heard::Quit => self.quit_asked(),
            Heard::Reopened => self.reopened(),
            Heard::Asked(asked) => self.person_asked(key, asked),
            Heard::Center(did) => self.person_on_center(did),
        }
    }

    /// The person asked for a tab of theirs with "+", in the project of group `group`: opened on
    /// a thread of its own, since the window's loop calls this and must never wait on the work.
    fn person_asked(self: &Arc<Self>, group: &str, asked: Asked) {
        let Some(person) = self.person(group) else {
            return;
        };
        let (desk, group) = (Arc::clone(self), crate::projects::key(person.project.id));
        // What the person asks is done in sight: the window goes to its project first (from the
        // Event Center it may be another's).
        if let Some(browser) = &self.browser {
            browser.go(&group);
        }
        let started = std::thread::Builder::new()
            .name("desk-person-asked".into())
            .spawn(move || {
                let call = Call {
                    owner: &person.owner,
                    project: &person.project,
                    agent: None,
                    cwd: &person.cwd,
                    vars: person.vars.as_deref(),
                    peek: false,
                };
                // The person is known as their project's own, as an agent is once it calls: what
                // they open is of that project wherever it is listed (an ended terminal's event).
                crate::lock(&desk.registry).known(&call);
                let done = match &asked {
                    Asked::Page(url) => {
                        let url = url.as_deref().unwrap_or(HOME);
                        (desk.open(&call, &json!({"kind": "browser", "url": url})))
                            .map(|_| ())
                            .map_err(|failure| failure.message().to_string())
                    }
                    Asked::Terminal(folder) => {
                        // Only a folder of the project (a crumb's), as it really is: no `..`
                        // or link leads out of it.
                        let inside = (folder.as_deref())
                            .and_then(|folder| folder.canonicalize().ok())
                            .filter(|folder| folder.starts_with(&person.project.path));
                        let cwd = inside.as_deref().unwrap_or(&person.cwd);
                        desk.person_terminal(&Call { cwd, ..call }, &group, "")
                    }
                    Asked::Agent(command) => desk.person_terminal(&call, &group, command),
                    Asked::Board => (desk.board(&call, &json!({"op": "show"})))
                        .map(|_| ())
                        .map_err(|failure| failure.message().to_string()),
                    Asked::Again(row) => desk.again(&call, &group, *row),
                    Asked::Project => desk.reopen_project(&call, &group),
                };
                if let Err(why) = done {
                    log::warn!("a {asked:?} the person asked for did not open: {why}");
                }
            });
        if let Err(why) = started {
            log::warn!("a tab the person asked for had no thread: {why}");
        }
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
                crate::lock(&self.seen).remove(&id);
            }
            self.gone(owner, &key);
        }
    }

    fn owner_of(&self, key: &str) -> Option<String> {
        crate::lock(&self.registry).owner_named(key)
    }

    /// Close every tab of `owner`; how many there were.
    pub fn end(&self, owner: &str) -> usize {
        let tabs = crate::lock(&self.registry).tabs_of(owner);
        for tab in &tabs {
            let _ = self.close_tab(tab);
        }
        crate::lock(&self.registry).forget(owner);
        // What the session asked the person to do goes with it.
        crate::lock(&self.asks).remove(&center::Subject::Session(owner.to_string()));
        tabs.len()
    }

    /// Every terminal ends; the window stays the process's to close.
    pub fn shutdown(&self) {
        // The tabs open now stay open in the ledger, their screens kept: the app opens them
        // again. Their sessions end below, and must not close them, nor the split they make.
        self.ending.store(true, std::sync::atomic::Ordering::SeqCst);
        self.keep_screens();
        crate::lock(&self.ledger).clear();
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
            (Some(_), _) => "not open yet",
            (None, _) => "none (no display, or --no-window)",
        };
        let registry = crate::lock(&self.registry);
        let mut out = format!(
            "window: {window}\nterminals: {}",
            crate::lock(&self.sessions).len()
        );
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
