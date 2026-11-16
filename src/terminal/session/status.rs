//! What a session's program says of its state (`terminal/status.rs`), on the session's side: its
//! query answered through the terminal whether or not a page shows it, whoever listens told of
//! each change, its lines read and their news told once until read, and its records let go as
//! the program goes (it ends, or the shell comes back in front of it) and as the person types in
//! the terminal.

use std::sync::Arc;

use super::super::status::{REPLY, Status};
use super::{Input, Session, Shared};

/// The most answers one read of output is given, as for the cursor's query: a program that asks
/// again and again in one breath is not reading what it is told.
const MOST_REPLIES: usize = 8;

impl Session {
    /// What the program says of its state: its root record first, then its parts'. Plain words
    /// of the program's, for the person and no agent.
    pub fn program_status(&self) -> Vec<Status> {
        self.shared.lock().screen.status().rows()
    }

    /// The lines its records have for the person (`terminal/status/lines.rs`), read: what
    /// changes after is news again.
    pub fn status_lines(&self) -> Vec<Status> {
        self.shared.lock().screen.status_mut().read_lines()
    }

    /// Whether the lines changed since they were last read, told once until they are read
    /// again: a program that flips its state tells its reader once, at the reader's pace.
    pub fn status_news(&self) -> bool {
        self.shared.lock().screen.status_mut().news()
    }

    /// The record that wants the person most, if there is one.
    pub fn most_urgent_status(&self) -> Option<Status> {
        self.shared.lock().screen.status().most_urgent()
    }

    /// `heard` each time what the program says of its state changes.
    pub fn on_status(&self, heard: impl Fn() + Send + Sync + 'static) {
        let mut state = self.shared.lock();
        state.on_status = Some(Arc::new(heard));
        state.statused = state.screen.status().changes();
    }
}

/// The program's `asked` queries answered: written to the terminal before anything the page
/// answers (its reply to a device attributes query comes through the window), which is the order
/// a program that asks waits for.
pub(super) fn answer(input: &Input, asked: usize) {
    for _ in 0..asked.min(MOST_REPLIES) {
        input.queue(REPLY);
    }
}

/// Whoever listens told that the records changed, once for each change seen.
pub(super) fn statused(shared: &Shared) {
    let heard = {
        let mut state = shared.lock();
        let now = state.screen.status().changes();
        if now == state.statused {
            return;
        }
        state.statused = now;
        state.on_status.clone()
    };
    if let Some(heard) = heard {
        heard();
    }
}

/// The group in front became `group`; the terminal's first program (its shell) is `pid`. When the
/// shell comes back in front, the program that was in front of it is gone, and what it left at
/// work goes with it; what is done or failed stays for the person.
pub(super) fn fronted(shared: &Shared, group: Option<libc::pid_t>, pid: u32) {
    let mut state = shared.lock();
    let back = group.is_some() && group == libc::pid_t::try_from(pid).ok();
    if back && state.front.group.is_some_and(|had| Some(had) != group) {
        state.screen.status_mut().drop_transient();
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
