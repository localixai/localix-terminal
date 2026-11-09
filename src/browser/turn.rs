//! Whose turn it is on the pages: while the person comments, an agent's action that would move a
//! page (a click, typing, keys, a load) waits for them, so nothing scrolls or navigates
//! under the element they are pointing at. Their Send, or the comment button off, gives the turn back.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// What an action that waited too long answers: the person is still commenting.
pub(super) const COMMENTING: &str = "the person is commenting on the page, so this action did not run \
     (it waited {waited} s). Their comments come with a later call or `--said`; act again after that";

#[derive(Default)]
pub(super) struct Turn {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    /// The person is commenting: actions wait.
    commenting: bool,
    /// Actions waiting for the person now.
    waiting: usize,
    /// The agent is acting on a page: the strip says so.
    acting: bool,
    /// When the agent's last action settled.
    settled: Option<Instant>,
}

/// How long after an action settles a page's move is still the agent's: a click's navigation
/// reaches WebKit's question a moment after the click's answer.
pub(super) const AFTER_ACTING: Duration = Duration::from_millis(750);

impl Turn {
    /// The agent began an action, or its action settled.
    pub(super) fn acting(&self, on: bool) {
        let mut state = crate::lock(&self.state);
        if state.acting && !on {
            state.settled = Some(Instant::now());
        }
        state.acting = on;
    }

    /// Whether a page moving now is the agent's doing: it acts, or its action settled a moment
    /// ago. Such a move never opens the person's browser.
    pub(super) fn agent_moves(&self) -> bool {
        self.agent_moves_at(Instant::now())
    }

    fn agent_moves_at(&self, now: Instant) -> bool {
        let state = crate::lock(&self.state);
        state.acting
            || (state.settled).is_some_and(|at| now.saturating_duration_since(at) < AFTER_ACTING)
    }

    /// Whether the agent acts now, and how many of its actions wait for the person.
    pub(super) fn now(&self) -> (bool, usize) {
        let state = crate::lock(&self.state);
        (state.acting, state.waiting)
    }

    /// Whether the person is commenting now, in some window.
    pub(super) fn is_commenting(&self) -> bool {
        crate::lock(&self.state).commenting
    }

    /// The person started or stopped commenting; stopping lets every waiting action go.
    pub(super) fn commenting(&self, on: bool) {
        crate::lock(&self.state).commenting = on;
        self.changed.notify_all();
    }

    /// Wait while the person comments, at most `limit`; `told` runs when the wait starts and
    /// when it ends (the strip says the agent waits). `Err` with how long it waited when the
    /// person was still commenting at the end.
    pub(super) fn wait(&self, limit: Duration, told: impl Fn()) -> Result<(), Duration> {
        let started = Instant::now();
        let mut state = crate::lock(&self.state);
        if !state.commenting {
            return Ok(());
        }
        state.waiting += 1;
        drop(state);
        told();
        let mut state = crate::lock(&self.state);
        while state.commenting && started.elapsed() < limit {
            let left = limit.saturating_sub(started.elapsed());
            state = crate::wait_timeout(&self.changed, state, left.min(Duration::from_millis(250)));
        }
        state.waiting = state.waiting.saturating_sub(1);
        let still = state.commenting;
        drop(state);
        told();
        if still {
            Err(started.elapsed())
        } else {
            Ok(())
        }
    }
}

/// The answer for an action that waited `waited` and did not run.
pub(super) fn refused(waited: Duration) -> String {
    COMMENTING.replace("{waited}", &waited.as_secs().to_string())
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
