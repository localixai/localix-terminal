//! A project's tab of a kind that draws itself (`crate::kind`): one tab of each kind in a
//! project, opened the first time it is asked for and shown after. The desk gives it a key, a
//! pane of the ledger and a panel of the window, and hands the kind what an agent calls and
//! what the person does in its page; what the tab holds and does is the kind's alone.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde_json::Value;

use crate::browser::{Asked, Pane, Panel};
use crate::kind::{About, Kind};
use crate::projects::ProjectId;

use super::actions::Tab;
use super::{Call, Desk, Failure, Output};

/// What the person did in a page that waits to be worked; more than a person does at once.
const WAITING: usize = 64;

/// What a tab's worker thread is handed, in the order it came.
enum Work {
    /// The person did this in the page.
    Posted(String),
    /// The page loaded, at first or again.
    Loaded,
}

/// A tab as its kind holds it: the panel it paints, and the desk for what it asks.
struct Held {
    key: String,
    /// The key of its project, as the window groups tabs.
    group: String,
    panel: Mutex<Option<Arc<Panel>>>,
    desk: Weak<Desk>,
}

impl crate::kind::Tab for Held {
    fn key(&self) -> &str {
        &self.key
    }

    fn paint(&self, frame: &str) {
        let panel = crate::lock(&self.panel).clone();
        if let Some(panel) = panel {
            panel.paint(frame);
        }
    }

    fn browse(&self, url: &str) {
        if let Some(desk) = self.desk.upgrade() {
            desk.person_asked(&self.group, Asked::Page(Some(url.to_string())));
        }
    }

    fn ask(&self, question: &crate::kind::Question) -> Option<crate::kind::Given> {
        let desk = self.desk.upgrade()?;
        desk.browser.as_ref()?.ask(question)
    }
}

/// A project's tab of a kind, as the desk holds it.
pub(super) struct Kept {
    held: Arc<Held>,
}

/// The kinds of tab that draw themselves, as they were listed when the desk was made, and each
/// project's tab of one, by the kind's name, while it is open.
pub(super) struct Tabs {
    pub(super) kinds: Vec<Arc<dyn Kind>>,
    held: Mutex<HashMap<(ProjectId, &'static str), Kept>>,
    next: AtomicU32,
}

impl Tabs {
    pub(super) fn new() -> Self {
        Self {
            kinds: crate::kind::all(),
            held: Mutex::default(),
            next: AtomicU32::new(1),
        }
    }
}

impl Desk {
    /// What the kind named `id` says of itself, when the desk has it.
    pub(super) fn kind_named(&self, id: &str) -> Option<&'static About> {
        (self.made.kinds.iter().map(|kind| kind.about())).find(|about| about.id == id)
    }

    /// The kind `about` is of, and its place among the kinds.
    fn kind_of(&self, about: &About) -> Result<(usize, Arc<dyn Kind>), String> {
        (self.made.kinds.iter().enumerate())
            .find(|(_, kind)| kind.about().id == about.id)
            .map(|(at, kind)| (at, Arc::clone(kind)))
            .ok_or_else(|| format!("there is no kind of tab `{}`", about.id))
    }

    /// A call on the project's tab of kind `about`: opened when there is none. With a field of
    /// the kind's own, the kind answers it; with none, the tab is shown and read.
    pub(super) fn made(
        &self,
        call: &Call,
        about: &'static About,
        args: &Value,
    ) -> Result<Output, Failure> {
        let (at, kind) = self.kind_of(about)?;
        let held = {
            let mut made = crate::lock(&self.made.held);
            match made.get(&(call.project.id, about.id)) {
                Some(kept) => Arc::clone(&kept.held),
                None => {
                    let held = self.new_made(call, at, &kind)?;
                    let kept = Kept {
                        held: Arc::clone(&held),
                    };
                    made.insert((call.project.id, about.id), kept);
                    held
                }
            }
        };
        let tab: Arc<dyn crate::kind::Tab> = Arc::clone(&held) as _;
        let own = args.as_object().is_some_and(|given| {
            (about.fields.iter()).any(|(name, _)| given.get(*name).is_some_and(|v| !v.is_null()))
        });
        if own {
            let said = kind.call(args, &tab)?;
            return Ok(Output {
                report: format!("{}: {}", held.key, said.report),
                state: said.state,
                shot: None,
            });
        }
        if self.browser.is_some() && self.reveal(call, &held.key).is_err() {
            // The window went with the tab (the person closed its last one): it has a tab again.
            self.own_made(&held.key, call, about);
            // The old panel let go first: it shares the new one's id.
            drop(crate::lock(&held.panel).take());
            let panel = self.made_panel(at, &kind, &held, self.quiet(call), &call.project.name);
            *crate::lock(&held.panel) = panel;
            self.reveal(call, &held.key)?;
        }
        Ok(Output {
            report: format!("{} ({}): {}", held.key, about.id, kind.summary()),
            state: Some(kind.text()),
            shot: None,
        })
    }

    /// Tab `key` is a pane of the call's project, of kind `about`.
    fn own_made(&self, key: &str, call: &Call, about: &'static About) {
        let facts = super::ledger::Facts {
            title: about.title,
            ..super::ledger::Facts::default()
        };
        self.own(key, call, crate::ledger::Kind::Made(about.id), facts);
    }

    /// The call's project's tab of `kind`, new: its key, its pane, its panel where a window shows.
    fn new_made(&self, call: &Call, at: usize, kind: &Arc<dyn Kind>) -> Result<Arc<Held>, String> {
        let about = kind.about();
        let n = self.made.next.fetch_add(1, Ordering::SeqCst);
        if crate::browser::made(at, n).is_none() {
            return Err(format!("no more tabs of kind `{}` in this run", about.id));
        }
        let key = format!("{}{n}", about.letter);
        self.own_made(&key, call, about);
        let held = Arc::new(Held {
            key,
            group: crate::projects::key(call.project.id),
            panel: Mutex::default(),
            desk: Weak::clone(&self.me),
        });
        let panel = self.made_panel(at, kind, &held, self.quiet(call), &call.project.name);
        *crate::lock(&held.panel) = panel;
        Ok(held)
    }

    /// The panel of tab `held`: what the person does in its page and each load of the page go to
    /// its kind on a thread of their own, in the order they came; the thread ends with the panel.
    ///
    /// `aside`, its tab opens beside the tab in sight and does not take the window.
    fn made_panel(
        &self,
        at: usize,
        kind: &Arc<dyn Kind>,
        held: &Arc<Held>,
        aside: bool,
        project: &str,
    ) -> Option<Arc<Panel>> {
        let browser = self.browser.as_ref()?;
        let n = held.key.get(1..)?.parse().ok()?;
        let id = crate::browser::made(at, n)?;
        let title = format!("desk · {} · {project}", kind.about().id);
        browser.group(&held.key, &held.group);
        if aside {
            browser.aside(&held.key);
        }
        let panel = Arc::new(browser.panel(id, &title, kind.page(&title)));
        let (to_worker, works) = std::sync::mpsc::sync_channel::<Work>(WAITING);
        let (kind, tab) = (Arc::clone(kind), Arc::downgrade(held));
        let worker = std::thread::Builder::new()
            .name("desk-person-kind".into())
            .spawn(move || {
                for work in works {
                    let Some(held) = tab.upgrade() else { return };
                    let tab: Arc<dyn crate::kind::Tab> = held;
                    match work {
                        Work::Posted(posted) => kind.posted(&posted, &tab),
                        Work::Loaded => kind.loaded(&tab),
                    }
                }
            });
        if let Err(why) = worker {
            log::warn!("{} takes no action from the person: {why}", held.key);
        }
        let loaded = to_worker.clone();
        panel.keyboard(Arc::new(move |posted: &str| {
            if to_worker
                .try_send(Work::Posted(posted.to_string()))
                .is_err()
            {
                log::warn!("what the person did in a tab was dropped: too much waits");
            }
        }));
        panel.sizing(Arc::new(move |pane, _, _| {
            if pane == Pane::Loaded && loaded.try_send(Work::Loaded).is_err() {
                log::warn!("a tab was not painted as its page loaded: too much waits");
            }
        }));
        Some(panel)
    }

    /// Whether a board, a terminal or a tab of a kind is in the window: it shows the window as
    /// much as a page does.
    pub(super) fn panels_open(&self) -> bool {
        !crate::lock(&self.sessions).is_empty()
            || (crate::lock(&self.boards).values()).any(|kept| kept.panel.is_some())
            || (crate::lock(&self.made.held).values())
                .any(|kept| crate::lock(&kept.held.panel).is_some())
    }

    /// Whether `tab` is project `project`'s tab of a kind that draws itself.
    pub(super) fn made_of(&self, project: ProjectId, tab: &Tab) -> bool {
        let super::actions::Kind::Made(about) = tab.kind else {
            return false;
        };
        (crate::lock(&self.made.held).get(&(project, about.id)))
            .is_some_and(|kept| kept.held.key == tab.key)
    }

    /// A tab of kind `about` as an agent reads it.
    pub(super) fn made_text(&self, about: &About) -> String {
        (self.kind_of(about)).map_or_else(|why| why, |(_, kind)| kind.text())
    }

    /// A tab of kind `about` in a few words, for `list`.
    pub(super) fn made_summary(&self, about: &About) -> String {
        (self.kind_of(about)).map_or_else(|_| String::new(), |(_, kind)| kind.summary())
    }

    /// Tab `key` closed: its panel goes with it. What its kind keeps is kept.
    pub(super) fn close_made(&self, key: &str) -> Result<String, String> {
        match self.forget_made(key) {
            true => Ok(format!("Closed {key}.")),
            false => Err(format!("there is no tab {key}")),
        }
    }

    /// Tab `key` is no tab of a kind that draws itself any more; whether it was one.
    pub(super) fn forget_made(&self, key: &str) -> bool {
        let mut made = crate::lock(&self.made.held);
        let before = made.len();
        made.retain(|_, kept| kept.held.key != key);
        made.len() != before
    }
}

#[cfg(test)]
#[path = "made_tests.rs"]
mod tests;
