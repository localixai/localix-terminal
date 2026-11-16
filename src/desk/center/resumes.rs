//! The agents of the run before that did not go on in their terminals as the app opened: one
//! that did not resume twice, with the person's Retry, and one whose session was not known (or
//! whose shell never said it was ready), with Continue. Each is one quiet line on its terminal;
//! one on its way is no news.

use super::sources::Source;
use super::{Do, Event, Kind, Leads};
use crate::desk::Desk;
use crate::desk::restore::Stage;

/// The terminals whose agent waits for the person's word to go on.
pub(super) struct Resumes;

impl Source for Resumes {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        // The past's lock is let go before the registry's is taken.
        let mut waiting: Vec<(String, &'static str, Stage)> = (crate::lock(&desk.past).resumes)
            .iter()
            .filter(|(_, resume)| matches!(resume.stage, Stage::Failed | Stage::Continue))
            .map(|(key, resume)| (key.clone(), resume.name, resume.stage))
            .collect();
        waiting.sort_by(|a, b| a.0.cmp(&b.0));
        let registry = crate::lock(&desk.registry);
        let mut events = vec![];
        for (key, name, stage) in waiting {
            let Some(owner) = registry.owner_named(&key) else {
                continue;
            };
            let (id, detail, label) = match stage {
                Stage::Failed => (format!("resume:{key}"), "Did not resume", "Retry"),
                Stage::Waiting | Stage::Typed | Stage::Continue => {
                    (format!("continue:{key}"), "Was open here", "Continue")
                }
            };
            let event = Event::new(id, Kind::Resume, &registry.group_of(&owner), &key)
                .says(name, detail.to_string())
                .button(label, Do::Resume { key: key.clone() })
                .button("Show", Do::Open(Leads::Tab(key.clone())));
            events.push(event);
        }
        events
    }
}

#[cfg(test)]
#[path = "resumes_tests.rs"]
mod tests;
