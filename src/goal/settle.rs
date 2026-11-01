//! How a goal run waits on its view: for a change after a WAIT, and for the view to settle
//! before the run reports it.

use std::time::Duration;

use crate::goal::run::GoalRun;
use crate::goal::step::{changed, fingerprint};
use crate::view::Observation;

/// A WAIT looks again every `WAIT_POLL` until the view changes, for at most `WAIT_BUSY` while the
/// view says it is working and `WAIT_IDLE` otherwise.
const WAIT_POLL: Duration = Duration::from_millis(250);
const WAIT_BUSY: Duration = Duration::from_secs(5);
const WAIT_IDLE: Duration = Duration::from_millis(1500);
/// How long a finished goal waits for its view to stop changing before it reports it.
const SETTLE_MAX: Duration = Duration::from_secs(2);

impl GoalRun<'_> {
    /// The view once it differs from `view`, or as it is after the longest wait: one decision per
    /// change instead of one per fixed pause.
    pub(super) fn wait_for_change(
        &self,
        view: &Observation,
        most: Option<Duration>,
    ) -> Option<Observation> {
        let most = most.unwrap_or(if view.busy { WAIT_BUSY } else { WAIT_IDLE });
        let deadline = std::time::Instant::now() + most;
        loop {
            (self.pause)(WAIT_POLL);
            let now = self.view.observe().ok();
            let done = now.as_ref().is_some_and(|now| changed(view, now));
            if done || std::time::Instant::now() >= deadline {
                return now;
            }
        }
    }

    /// The view to hand back, once late content has arrived (prices fill in, a list finishes
    /// rendering): two identical looks in a row on a view that is not busy, for at most
    /// `SETTLE_MAX`, so the caller does not judge a view half-loaded.
    pub(super) fn settled(&self, last: Option<Observation>) -> Option<Observation> {
        let deadline = std::time::Instant::now() + SETTLE_MAX;
        let mut previous = last.map(Ok).unwrap_or_else(|| self.view.observe()).ok()?;
        loop {
            (self.pause)(WAIT_POLL);
            let Ok(now) = self.view.observe() else {
                return Some(previous);
            };
            let quiet = fingerprint(&now) == fingerprint(&previous) && !now.busy;
            if quiet || std::time::Instant::now() >= deadline {
                return Some(now);
            }
            previous = now;
        }
    }
}
