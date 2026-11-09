//! The Event Center: what in the desk waits for the person, gathered from its owners and shown
//! under the strip's end (its shortcut opens it), each with buttons that act on it.
//!
//! It is a projection. It keeps nothing the owners do not hold (the boards, the terminals, how
//! what the agent asked), so it is gathered again whole when a notice says
//! something changed, and is the same after a restart as the owners are. All it keeps of its own
//! is the person's: which events they have seen, and which they dismissed. Its buttons take the
//! paths the person's own clicks take (the board's Accept, a tab's x), so an event is no second
//! way to do anything.
//!
//! What an event is (`Kind`) says how it is shown: its section, its tone, whether it wants the
//! person. Its buttons say what they are for (`Role`). So the window draws an event as it comes and
//! knows no kind of its own; a new source is a type in `sources` and nothing else.

mod event;
mod journal;
mod sources;

use event::{Do, Event, Kind};

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use serde_json::json;

use super::Desk;
use crate::board;
use crate::browser::{Asked, CenterDid};
use crate::feed::{Consumer, Entity, Kind as Of, Notice, Op};

/// Who the agent asks the person for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Subject {
    /// Something to do on a tab: kept until they answer, it is taken back, or the tab closes.
    Tab(String),
    /// The owner's session waits for them: kept until it goes on.
    Session(String),
}

impl Subject {
    /// The words that say where: `on w1`, `for this session`.
    fn place(&self) -> String {
        match self {
            Subject::Tab(tab) => format!("on {tab}"),
            Subject::Session(_) => "for this session".to_string(),
        }
    }

    /// How an ask of it ends, as the agent is told when it asks.
    fn ends(&self) -> &'static str {
        match self {
            Subject::Tab(_) => {
                "their Done or comment reaches you as their notes do, and it goes with its tab"
            }
            Subject::Session(_) => "it stays until you go on",
        }
    }
}

/// What the agent asked; one for each subject, a new one replacing it.
pub(super) struct Ask {
    /// Its number: a new ask is a new event, even within the same second.
    n: u64,
    owner: String,
    words: String,
    at: u64,
}

/// The number of the last ask.
static ASKED: AtomicU64 = AtomicU64::new(0);

/// The longest ask kept, in characters: a line, not a letter.
const ASK_MOST: usize = 280;

/// The person's part of the center, and what it showed last.
#[derive(Default)]
pub(super) struct Center {
    /// The number the strip says back for each event shown: a handle of this run, not kept.
    numbers: HashMap<String, u64>,
    next: u64,
    shown: Vec<Event>,
    /// The events as last gathered, before the person's marks: the same again is no change,
    /// and the journal is not written.
    gathered: Vec<Event>,
    /// What the window holds is not `shown` any more (the person marked it): it is painted again
    /// even when the events gathered are the same, an empty list after Clear All among them.
    stale: bool,
}

/// The center's consumer of the feed: whatever changed, it is gathered again.
pub(super) struct Projector {
    pub(super) desk: Weak<Desk>,
}

impl Consumer for Projector {
    fn name(&self) -> &'static str {
        "center"
    }

    fn kind(&self) -> Of {
        Of::Projector
    }

    fn handle(&mut self, notice: &Notice) -> Result<(), String> {
        let Some(desk) = self.desk.upgrade() else {
            return Ok(());
        };
        match &notice.entity {
            // A tab closed: what was kept for it goes with it.
            Entity::Pane { key, .. } if notice.op == Op::Deleted => {
                desk.tab_gone(key);
                desk.paint_center();
            }
            Entity::Board { .. }
            | Entity::Terminal { .. }
            | Entity::Ask { .. }
            | Entity::Pane { .. } => desk.paint_center(),
            // Where a pane is shown is not the person's to act on.
            Entity::Window { .. } | Entity::Workspace { .. } | Entity::Tab { .. } => {}
            // A note waiting for the agent is not the person's to act on.
            Entity::Said { .. } => {}
            // A project's history and the projects known are what the window lists, not events.
            Entity::Ledger { .. } | Entity::Projects => {}
        }
        Ok(())
    }

    fn reconcile(&mut self) {
        if let Some(desk) = self.desk.upgrade() {
            desk.paint_center();
        }
    }
}

impl Desk {
    /// The agent of `owner` asks the person `words` for `subject` (none: it takes that back);
    /// what it is told back.
    pub(super) fn ask(&self, owner: &str, subject: Subject, words: Option<&str>) -> String {
        let place = subject.place();
        let mut asks = crate::lock(&self.asks);
        let said = match words {
            Some(words) => {
                let ends = subject.ends();
                let ask = Ask {
                    n: ASKED.fetch_add(1, Ordering::Relaxed) + 1,
                    owner: owner.to_string(),
                    words: words.chars().take(ASK_MOST).collect(),
                    at: board::now(),
                };
                asks.insert(subject, ask);
                format!(
                    "Asked the person {place}: it waits for them in the Event Center, and {ends}. \
                     Another ask replaces it; `\"ask\": \"\"` takes it back."
                )
            }
            None if asks.remove(&subject).is_some() => format!("The ask {place} is taken back."),
            None => return format!("Nothing was asked {place}."),
        };
        let owner = owner.to_string();
        self.feed.record(&Entity::Ask { owner }, Op::Upserted);
        said
    }

    /// A run begins: what the last one left open of its own (asks, programs that ended) is past.
    pub(super) fn settle_last_run(&self) {
        let now = board::now();
        if let Err(why) = (self.feed.store()).write(|tx| journal::settle_run_local(tx, now)) {
            log::warn!("desk: the last run's events stay open in the journal: {why}");
        }
    }

    /// Tab `key` closed: what was asked on it goes.
    fn tab_gone(&self, key: &str) {
        crate::lock(&self.asks).remove(&Subject::Tab(key.to_string()));
    }

    /// The center gathered again and, if it changed, shown.
    pub(super) fn paint_center(&self) {
        // One paint at a time, from its gathering to the window: the feed's consumer and the
        // person's clicks both paint, and an older gathering never paints over a newer one.
        let _painting = crate::lock(&self.painting);
        if let (Some(events), Some(browser)) = (self.regathered(), &self.browser) {
            // Each says its project by the name the switcher shows.
            let names = self.project_names();
            let mut events = json!(events);
            for event in events.as_array_mut().into_iter().flatten() {
                let name = (event.get("group").and_then(serde_json::Value::as_str))
                    .and_then(|key| names.get(key).cloned());
                if let Some(name) = name {
                    event["group"] = json!(name);
                }
            }
            browser.center(events.to_string());
        }
    }

    /// The events as the center shows them now (numbered, the dismissed left out), when they
    /// are not what it showed last. Called while painting: no other gathering runs meanwhile.
    fn regathered(&self) -> Option<Vec<Event>> {
        let gathered: Vec<Event> = (sources::SOURCES.iter())
            .flat_map(|source| source.gather(self))
            .collect();
        let now = board::now();
        {
            let mut center = crate::lock(&self.center);
            if gathered == center.gathered && !center.stale {
                return None;
            }
            center.gathered.clone_from(&gathered);
        }
        // Written with the center's lock let go: what the person clicks reads it meanwhile.
        let kept = (self.feed.store())
            .write(|tx| journal::record(tx, &gathered, now))
            .unwrap_or_else(|why| {
                log::warn!("desk: the Event Center's journal was not written: {why}");
                HashMap::new()
            });
        let mut center = crate::lock(&self.center);
        let center = &mut *center;
        let ids: HashSet<&str> = gathered.iter().map(|event| event.id.as_str()).collect();
        center.numbers.retain(|id, _| ids.contains(id.as_str()));
        let mut events: Vec<Event> = gathered
            .into_iter()
            .filter_map(|event| {
                let marks = kept.get(&event.id).copied().unwrap_or_default();
                if marks.dismissed {
                    return None;
                }
                let mut event = event.button("Dismiss", Do::Dismiss);
                event.n = *center.numbers.entry(event.id.clone()).or_insert_with(|| {
                    center.next += 1;
                    center.next
                });
                if event.at == 0 {
                    event.at = if marks.first == 0 { now } else { marks.first };
                }
                event.unseen = !marks.seen;
                Some(event)
            })
            .collect();
        events.sort_by(|a, b| (a.kind, b.at, b.n).cmp(&(b.kind, a.at, a.n)));
        if events == center.shown && !center.stale {
            return None;
        }
        center.stale = false;
        center.shown.clone_from(&events);
        Some(events)
    }

    /// The events `ids` marked as the person did, in the journal.
    fn mark(&self, ids: &[String], mark: journal::Mark) {
        // Not while a paint runs: it would read the journal before the marks, then take the
        // window as up to date.
        let _painting = crate::lock(&self.painting);
        if let Err(why) = (self.feed.store()).write(|tx| journal::mark(tx, ids, mark)) {
            log::warn!("desk: the Event Center's journal was not marked: {why}");
        }
        crate::lock(&self.center).stale = true;
    }

    /// What the person did in the center: done on a thread of its own, since the window's loop
    /// tells it and must never wait on the work.
    pub(super) fn person_on_center(self: &Arc<Self>, did: CenterDid) {
        let desk = Arc::clone(self);
        let started = std::thread::Builder::new()
            .name("desk-center".into())
            .spawn(move || match did {
                CenterDid::Opened => desk.center_seen(),
                CenterDid::Chose { event, button } => desk.center_do(event, button),
                CenterDid::Cleared => desk.center_cleared(),
            });
        if let Err(error) = started {
            log::warn!("desk: the Event Center could not act: {error}");
        }
    }

    /// All the center shows is seen: its events are no longer new.
    fn center_seen(&self) {
        let ids = self.center_shown();
        self.mark(&ids, journal::Mark::Seen);
        self.paint_center();
    }

    /// The ids of what the center shows, to be shown again as marked.
    fn center_shown(&self) -> Vec<String> {
        let mut center = crate::lock(&self.center);
        center.stale = true;
        center.shown.iter().map(|event| event.id.clone()).collect()
    }

    /// All the center shows is dismissed: each comes back only when it changes.
    fn center_cleared(&self) {
        let ids = self.center_shown();
        self.mark(&ids, journal::Mark::Dismissed);
        self.paint_center();
    }

    /// Button `at` of the event numbered `n`.
    fn center_do(self: &Arc<Self>, n: u64, at: usize) {
        let found = {
            let center = crate::lock(&self.center);
            (center.shown.iter())
                .find(|event| event.n == n)
                .and_then(|event| Some((event.id.clone(), event.buttons.get(at)?.does.clone())))
        };
        let Some((id, does)) = found else {
            return;
        };
        match does {
            Do::Accept { project, card } => {
                let posted = json!({"op": "accept", "card": card}).to_string();
                self.person_on_board(project, &posted);
            }
            Do::Unblock { project, card } => {
                let posted = json!({"op": "unblock", "card": card}).to_string();
                self.person_on_board(project, &posted);
            }
            Do::Board { group } => self.person_asked(&group, Asked::Board),
            Do::Show { key } => {
                if let Some(browser) = &self.browser {
                    let _ = browser.reveal(&key);
                }
            }
            Do::Close { key } => {
                let _ = self.close_tab(&key);
            }
            Do::Answer { tab } => {
                let ask = crate::lock(&self.asks).remove(&Subject::Tab(tab.clone()));
                if let Some(ask) = ask {
                    let words = format!("The person did what you asked on {tab}: {}", ask.words);
                    self.hear(&tab, &words, true);
                }
            }
            Do::Dismiss => self.mark(&[id], journal::Mark::Dismissed),
        }
        self.paint_center();
    }
}

#[cfg(test)]
#[path = "center_tests.rs"]
mod tests;
