//! The window's offer bar: a row under the strip that offers the person something at the moment
//! it fits, with a button that does it and Not Now (the app keeps their answers: offers.rs). Today one
//! source: an agent running in the terminal shown whose integration is off (setup.rs says which
//! and what to offer). Another is a function here that says its offer for the tab shown.
//!
//! An integration's state is read on a thread (it runs the person's programs) once a run and
//! kept; the bar is drawn again when it is known, and while one is being turned on it says so.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tao::event_loop::EventLoopProxy;

use super::host::{Cmd, Shared};
use super::strip::Key;

/// The ids of the offers of an integration: this, then the integration's.
const OF_INTEGRATION: &str = "integration:";

/// The bar's height, in points, while it shows.
pub(super) const ROW: f64 = 36.0;

/// What the bar shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Bar {
    /// The offer's id, as offers.rs keeps the answers.
    #[serde(skip)]
    pub(super) id: String,
    /// The coding agent it is about, whose mark it shows.
    pub(super) agent: Option<&'static str>,
    pub(super) says: String,
    /// The button that does it; none while it is being done, or once it is.
    pub(super) take: Option<String>,
    /// The button that puts it off, or closes what was done.
    pub(super) later: Option<String>,
    /// It is being done.
    pub(super) busy: bool,
    /// What was done failed.
    pub(super) failed: bool,
}

/// What the window knows for its offers.
#[derive(Default)]
pub(super) struct Offers {
    /// The program in front in each terminal, by key, when it is a coding agent (its id).
    runs: Mutex<HashMap<String, &'static str>>,
    /// Whether each integration is on, once read; none while it is being read.
    on: Mutex<HashMap<&'static str, Option<bool>>>,
    /// A bar that outlives the tab shown: an offer being done, or what came of it.
    held: Mutex<Option<Bar>>,
    /// Where the person's answers are kept, once the app says: till then nothing is offered.
    answers: Mutex<Option<Arc<dyn super::Answers>>>,
}

impl Offers {
    /// The person's answers are kept by `answers`.
    pub(super) fn keep(&self, answers: Arc<dyn super::Answers>) {
        *crate::lock(&self.answers) = Some(answers);
    }

    /// Whether offer `id` may be shown now: never before the answers are known.
    fn due(&self, id: &str) -> bool {
        let answers = crate::lock(&self.answers).clone();
        answers.is_some_and(|answers| answers.due(id))
    }

    /// The person answered offer `id`: kept.
    fn answered(&self, id: &str, taken: bool) {
        let answers = crate::lock(&self.answers).clone();
        if let Some(answers) = answers {
            answers.answered(id, taken);
        }
    }

    /// Terminal `key` runs `program` in front now (none: nothing known); whether its agent
    /// changed.
    pub(super) fn runs(&self, key: &str, program: Option<&str>) -> bool {
        let agent = program
            .and_then(super::agents::of_command)
            .map(|agent| agent.id);
        let mut runs = crate::lock(&self.runs);
        let before = match agent {
            Some(agent) => runs.insert(key.to_string(), agent),
            None => runs.remove(key),
        };
        drop(runs);
        // An agent just started: its integration is read again, as it is now (the person may
        // have taken it out since, here or with the agent's own commands).
        if let Some(integration) = agent
            .filter(|_| before != agent)
            .and_then(crate::setup::for_agent)
        {
            crate::lock(&self.on).remove(integration.id);
        }
        before != agent
    }

    /// The coding agent terminal `key` runs in front, if it is known.
    pub(super) fn agent(&self, key: &str) -> Option<&'static str> {
        crate::lock(&self.runs).get(key).copied()
    }

    /// The bar for tab `shown` now, if any. An integration not read yet is read on a thread,
    /// and the strip drawn again (`proxy`) when it is.
    pub(super) fn bar(&self, shown: Key, proxy: &EventLoopProxy<Cmd>) -> Option<Bar> {
        let held = crate::lock(&self.held).clone();
        if held.is_some() {
            return held;
        }
        let Key::Term(_) = shown else {
            return None;
        };
        let integration = crate::setup::for_agent(self.agent(&shown.name())?)?;
        let offered = integration.offer.as_ref()?;
        let id = format!("{OF_INTEGRATION}{}", integration.id);
        if !self.due(&id) {
            return None;
        }
        let known = crate::lock(&self.on).get(integration.id).copied();
        match known {
            Some(Some(false)) => Some(Bar {
                id,
                agent: integration.agent,
                says: offered.says.to_string(),
                take: Some(offered.take.to_string()),
                later: Some("Not Now".to_string()),
                busy: false,
                failed: false,
            }),
            Some(Some(true) | None) => None,
            None => {
                self.read(integration.id, proxy.clone());
                None
            }
        }
    }

    /// Integration `id`'s state read on a thread; the window is told (`Cmd::Offered`).
    fn read(&self, id: &'static str, proxy: EventLoopProxy<Cmd>) {
        crate::lock(&self.on).insert(id, None);
        let read = std::thread::Builder::new()
            .name("desk-offer-read".into())
            .spawn(move || {
                let _ = proxy.send_event(Cmd::Offered(id, !crate::setup::offered(id)));
            });
        if let Err(why) = read {
            log::warn!("offers: integration {id} was not read (no thread: {why})");
        }
    }

    /// Integration `id` is on, or off, as read.
    pub(super) fn known(&self, id: &'static str, on: bool) {
        crate::lock(&self.on).insert(id, Some(on));
    }

    /// The integrations as Settings read them (`setup::states`): what the bar knows of each.
    pub(super) fn read_all(&self, states: &serde_json::Value) {
        for integration in crate::setup::INTEGRATIONS.iter() {
            let state = (states.as_array().into_iter().flatten())
                .find(|state| state["id"] == integration.id);
            // Off and not taken out by the person: offered; anything else is not.
            match state {
                Some(state) => {
                    let offered = state["state"] == "off" && state["declined"] != true;
                    self.known(integration.id, !offered);
                }
                None => drop(crate::lock(&self.on).remove(integration.id)),
            }
        }
    }

    /// The person clicked the bar's button that does it (`take`), or the other; what to do on a
    /// thread, if anything: the offer to take.
    pub(super) fn clicked(&self, bar: Option<&Bar>, take: bool) -> Option<String> {
        let bar = bar?;
        let mut held = crate::lock(&self.held);
        match (take, bar.take.is_some()) {
            (true, true) => {
                *held = Some(Bar {
                    take: None,
                    later: None,
                    busy: true,
                    failed: false,
                    says: "Adding…".to_string(),
                    ..bar.clone()
                });
                Some(bar.id.clone())
            }
            (true, false) => None,
            // What came of it closed; an offer not done put off.
            (false, _) => {
                *held = None;
                if !bar.busy && bar.take.is_some() {
                    self.answered(&bar.id, false);
                }
                None
            }
        }
    }

    /// The offer `id` was done: what came of it, held until the person closes it.
    pub(super) fn done(&self, id: &str, done: Result<String, String>) {
        let integration = (crate::setup::INTEGRATIONS.iter())
            .find(|known| id.strip_prefix(OF_INTEGRATION) == Some(known.id));
        let bar = match done {
            Ok(_) => {
                self.answered(id, true);
                if let Some(integration) = integration {
                    self.known(integration.id, true);
                }
                let after = integration.and_then(|known| known.offer.as_ref());
                Bar {
                    id: id.to_string(),
                    agent: integration.and_then(|known| known.agent),
                    says: after.map_or("Added.", |offered| offered.after).to_string(),
                    take: None,
                    later: Some("OK".to_string()),
                    busy: false,
                    failed: false,
                }
            }
            Err(why) => Bar {
                id: id.to_string(),
                agent: integration.and_then(|known| known.agent),
                says: format!("Not added: {why}"),
                take: Some("Try Again".to_string()),
                later: Some("Not Now".to_string()),
                busy: false,
                failed: true,
            },
        };
        *crate::lock(&self.held) = Some(bar);
    }
}

/// Offer `id` done on a thread (an integration turned on), the window told what came of it.
pub(super) fn take(id: String, shared: &Arc<Shared>, proxy: &EventLoopProxy<Cmd>) {
    let (shared, proxy) = (Arc::downgrade(shared), proxy.clone());
    let started = std::thread::Builder::new()
        .name("desk-offer-take".into())
        .spawn(move || {
            let integration = id.strip_prefix(OF_INTEGRATION).unwrap_or(&id).to_string();
            let done = crate::setup::turn(&integration, true);
            if let Some(shared) = shared.upgrade() {
                shared.offers.done(&id, done);
                let _ = proxy.send_event(Cmd::Strip);
            }
        });
    if let Err(why) = started {
        log::warn!("offers: {why}");
    }
}

/// The bar's call in the strip's page: `O(bar)`, or `O(null)` to hide it.
pub(super) fn script(bar: Option<&Bar>) -> String {
    format!(
        "O({})",
        serde_json::to_string(&bar).unwrap_or_else(|_| "null".into())
    )
}

#[cfg(test)]
#[path = "offer_tests.rs"]
mod tests;
