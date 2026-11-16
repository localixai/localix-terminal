//! The window's offer bar: a row under the strip that offers the person something at the moment
//! it fits, with a button that does it and Not Now (the app keeps their answers: offers.rs). Today
//! one source: an update that is ready (updates.rs). Another is a function here that says its
//! offer for the tab shown. The window also keeps here which coding agent each terminal runs in
//! front, which the side marks it by.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

/// How long what the store said of an offer is believed: the bar is drawn on every refresh, and
/// the store is the one the app's writers share, so it is not asked each time. Only the person's
/// own answer changes it, and that is taken at once.
const DUE_FOR: Duration = Duration::from_secs(5);

/// The bar's height, in points, while it shows.
pub(super) const ROW: f64 = 36.0;

/// What the bar shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Bar {
    /// The offer's id, as offers.rs keeps the answers.
    #[serde(skip)]
    pub(super) id: String,
    pub(super) says: String,
    /// The button that does it.
    pub(super) take: Option<String>,
    /// The button that puts it off.
    pub(super) later: Option<String>,
}

/// What the window knows for its offers.
#[derive(Default)]
pub(super) struct Offers {
    /// The program in front in each terminal, by key, when it is a coding agent (its id).
    runs: Mutex<HashMap<String, &'static str>>,
    /// Where the person's answers are kept, once the app says: till then nothing is offered.
    answers: Mutex<Option<Arc<dyn super::Answers>>>,
    /// What the store last said of whether each offer is due, and when.
    dues: Mutex<HashMap<String, (Instant, bool)>>,
}

impl Offers {
    /// The person's answers are kept by `answers`.
    pub(super) fn keep(&self, answers: Arc<dyn super::Answers>) {
        *crate::lock(&self.answers) = Some(answers);
        crate::lock(&self.dues).clear();
    }

    /// Whether offer `id` may be shown now: never before the answers are known.
    pub(super) fn due(&self, id: &str) -> bool {
        let answers = crate::lock(&self.answers).clone();
        let Some(answers) = answers else {
            return false;
        };
        let known = crate::lock(&self.dues).get(id).copied();
        if let Some((at, due)) = known
            && at.elapsed() < DUE_FOR
        {
            return due;
        }
        let due = answers.due(id);
        crate::lock(&self.dues).insert(id.to_string(), (Instant::now(), due));
        due
    }

    /// The person answered the announcement `id` of what `offer` offers: it is not made again,
    /// and put off (not `taken`), the offer waits as one answered Not Now does.
    pub(super) fn announced(&self, id: &str, offer: &str, taken: bool) {
        self.answered(id, true);
        if !taken {
            self.answered(offer, false);
        }
    }

    /// The person answered offer `id`: kept, on a thread (the store may be busy with another
    /// writer's work). Answered, it is not due: that is known at once.
    fn answered(&self, id: &str, taken: bool) {
        let answers = crate::lock(&self.answers).clone();
        let Some(answers) = answers else {
            return;
        };
        crate::lock(&self.dues).insert(id.to_string(), (Instant::now(), false));
        let id = id.to_string();
        let kept = std::thread::Builder::new()
            .name("desk-offer-answer".into())
            .spawn(move || answers.answered(&id, taken));
        if let Err(why) = kept {
            log::warn!("offers: an answer was not kept (no thread: {why})");
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
        before != agent
    }

    /// The coding agent terminal `key` runs in front, if it is known.
    pub(super) fn agent(&self, key: &str) -> Option<&'static str> {
        crate::lock(&self.runs).get(key).copied()
    }

    /// The bar for the tab shown now, if any: an update that is ready is offered on any tab.
    pub(super) fn bar(&self) -> Option<Bar> {
        let (id, says) = super::updates::offer(&crate::update::app().told())?;
        // Announced in a dialog, it is not said twice at once.
        (self.due(&id) && !super::dialog::up()).then(|| Bar {
            id,
            says,
            take: Some("Restart to Update".to_string()),
            later: Some("Not Now".to_string()),
        })
    }

    /// The person clicked the bar's button that does it (`take`), or the other: the offer to
    /// take, which is the desk's to carry out (it may ask first); put off, it waits.
    pub(super) fn clicked(&self, bar: Option<&Bar>, take: bool) -> Option<String> {
        let bar = bar?;
        match (take, bar.take.is_some()) {
            (true, true) => Some(bar.id.clone()),
            (true, false) => None,
            (false, _) => {
                if bar.take.is_some() {
                    self.answered(&bar.id, false);
                }
                None
            }
        }
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
