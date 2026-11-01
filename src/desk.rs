//! The desk: the `window` tool's actions over the window's tabs, kept per owner. An owner is a
//! project (or a name an agent chose with `--as`); it opens, acts in, pictures and closes its own
//! tabs, and never sees another's. Pages are `w1`, `w2`…; terminal sessions `t1`, `t2`…. The
//! window is the person's: they may show, close or comment on any tab.
//!
//! Here, the desk itself and how a call is answered; `open`, `interact` and the rest are
//! in `actions`, a page's tab in `web`, a terminal's in `term`, a `goal` in `goal`.

mod actions;
mod args;
mod changes;
mod events;
mod goal;
mod open;
mod registry;
mod sealed;
mod term;
mod told;
mod web;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, Weak};

use serde_json::{Value, json};

use crate::browser::{Browser, Heard, Page, Shot};
use crate::served::Served;
use crate::terminal::Session;

pub use args::{description, schema};
pub use events::{Events, Quiet, said_block};
use registry::Registry;
pub use registry::Said;
pub use web::project;

/// Told a step of a `goal`: the tab it acts on, and the step.
pub type Progress<'a> = &'a (dyn Fn(&str, &str) + Sync);

/// Who is calling, from where, and with what the call may need of the caller.
#[derive(Clone, Copy)]
pub struct Call<'a> {
    pub owner: &'a str,
    /// The caller's directory: relative paths start here.
    pub cwd: &'a Path,
    /// The caller's environment, for a program started in a terminal.
    pub vars: Option<&'a [(String, String)]>,
    /// The caller's OpenRouter key, for a `goal`.
    pub key: Option<&'a str>,
    /// Told each step a `goal` takes, with the tab it acts on.
    pub progress: Option<Progress<'a>>,
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
    next_terminal: AtomicU32,
    registry: Mutex<Registry>,
    served: Served,
    events: Arc<dyn Events>,
    /// Each web tab's page as the caller last read it whole.
    seen: Mutex<HashMap<u32, changes::Seen>>,
    /// One lock per web tab: work in one never waits on another's.
    locks: Mutex<HashMap<u32, Arc<Mutex<()>>>>,
    /// Opening and closing tabs, one at a time.
    tabs_change: Mutex<()>,
    shots: PathBuf,
    /// The secrets calls had put in: no answer shows their values.
    vault: crate::secrets::Vault,
    /// Per browser tab, the paying controls its goals clicked: a later goal clicks one again only
    /// when it asks to do it again.
    paid: Mutex<HashMap<u32, Vec<String>>>,
}

impl Desk {
    /// A desk over `browser`'s window, or with no window at all (terminals only).
    pub fn new(
        browser: Option<Browser>,
        headless: bool,
        shots: PathBuf,
        events: Arc<dyn Events>,
    ) -> Arc<Self> {
        let desk = Arc::new_cyclic(|weak: &Weak<Self>| {
            if let Some(browser) = &browser {
                let weak = Weak::clone(weak);
                browser.listen(move |key, heard| {
                    if let Some(desk) = weak.upgrade() {
                        desk.heard(key, heard);
                    }
                });
            }
            Self {
                page: browser.clone().map(|browser| Page::new(browser, headless)),
                browser,
                sessions: Mutex::default(),
                next_terminal: AtomicU32::new(1),
                registry: Mutex::default(),
                served: Served::default(),
                events,
                seen: Mutex::default(),
                locks: Mutex::default(),
                tabs_change: Mutex::new(()),
                shots,
                vault: crate::secrets::Vault::default(),
                paid: Mutex::default(),
            }
        });
        // A picture someone asked for earlier is not worth keeping for long.
        crate::shots::prune(&desk.shots);
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
            said: self.said(call.owner).map(|said| self.vault.mask(&said)),
            shot: out.shot,
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

    /// Say a note to those who subscribed to `owner`'s.
    fn announce(&self, owner: &str, said: &Said) {
        self.events
            .publish("comment/added", Some(owner), said.note(owner));
    }

    /// What the person did in the window: a comment, a closed tab, a terminal asked for.
    fn heard(self: &Arc<Self>, key: &str, heard: Heard<'_>) {
        match heard {
            Heard::Sent(words) | Heard::Did(words) => self.kept(key, words),
            Heard::Closed(words) => {
                self.kept(key, words);
                self.events.publish(
                    "tab/closed",
                    self.owner_of(key).as_deref(),
                    json!({"tab": key}),
                );
            }
            Heard::WindowClosed(_) => {
                self.prune_pages();
                self.events.publish("window/closed", None, json!({}));
            }
            Heard::Terminal => self.person_terminal(key),
        }
    }

    /// The person sends `words` about tab `key`, as they would from the window: for tests, which
    /// have no window to send from.
    #[cfg(feature = "test-daemon")]
    pub fn person_says(&self, key: &str, words: &str) {
        self.kept(key, words);
    }

    /// `words` kept for the owner of tab `key`, and told to those who subscribed.
    fn kept(&self, key: &str, words: &str) {
        let heard = crate::lock(&self.registry).hear(key, words);
        if let Some((owner, said)) = heard {
            self.announce(&owner, &said);
        }
    }

    /// The pages the window no longer has (the person closed it, or the tab) forgotten as a
    /// closed tab is: a call on one is told there is no such tab, `list` names none, and the
    /// owner hears `tab/closed`.
    fn prune_pages(&self) {
        let Some(page) = &self.page else { return };
        let alive: Vec<String> = if page.is_open() {
            match self.browser.as_ref().and_then(Browser::listing) {
                Some(listing) => listing.into_iter().map(|tab| tab.id).collect(),
                None => return,
            }
        } else {
            vec![]
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
                crate::lock(&self.paid).remove(&id);
            }
            self.events
                .publish("tab/closed", owner.as_deref(), json!({"tab": key}));
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
        tabs.len()
    }

    /// Every terminal ends; the window stays the process's to close.
    pub fn shutdown(&self) {
        let sessions = std::mem::take(&mut *crate::lock(&self.sessions));
        sessions.values().for_each(|session| session.close());
    }

    /// The window and every owner's tabs, for `serve status`.
    pub fn status(&self) -> String {
        let window = match (&self.page, &self.browser) {
            (Some(page), _) if page.is_open() => "open",
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
