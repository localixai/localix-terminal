//! Wait for screen text, quiet output or process exit using the session's condition variable.

use super::{Session, Until};
use std::time::{Duration, Instant};

/// Output that stays still this long has settled.
const SETTLE: Duration = Duration::from_millis(400);
/// A wait with nothing named to wait for ends after this much: settled, if the output stopped, else
/// still changing (a program that prints nothing is quiet).
const QUIET: Duration = Duration::from_millis(1500);

impl Session {
    /// How many times `needle` shows on the screen now.
    pub fn count(&self, needle: &str) -> usize {
        match needle.is_empty() {
            true => 0,
            false => self
                .shared
                .lock()
                .screen
                .search_text()
                .matches(needle)
                .count(),
        }
    }

    /// Wait for `needle` to show more than `before` times, for the output to settle, or for the
    /// program to end; at most `most`.
    pub fn wait(&self, needle: Option<&str>, before: usize, most: Duration) -> Until {
        let needle = needle.filter(|needle| !needle.is_empty());
        let start = Instant::now();
        let mut state = self.shared.lock();
        let first = state.version;
        // The screen is looked in again only when output came in since.
        let mut looked = None;
        loop {
            if let Some(needle) = needle
                && looked != Some(state.version)
            {
                looked = Some(state.version);
                if state.screen.search_text().matches(needle).count() > before {
                    return Until::Matched;
                }
            }
            if state.ended {
                return Until::Exited;
            }
            let idle = state.changed_at.elapsed();
            let moved = state.version != first;
            if needle.is_none() {
                if moved && idle >= SETTLE {
                    return Until::Settled;
                }
                if start.elapsed() >= QUIET {
                    // Nothing came in since the call began: quiet. Output still coming: not done.
                    return if moved {
                        Until::Deadline
                    } else {
                        Until::Settled
                    };
                }
            }
            let left = most.saturating_sub(start.elapsed());
            if left.is_zero() {
                return Until::Deadline;
            }
            let slice = left.min(Duration::from_millis(50));
            state = crate::wait_timeout(&self.shared.wake, state, slice);
        }
    }
}

impl Until {
    /// How the wait ended, in words: for `wanted`, the text waited for, within `most`.
    pub fn said(self, wanted: Option<&str>, most: Duration) -> String {
        match (self, wanted) {
            (Self::Matched, Some(wanted)) => format!("\"{wanted}\" showed."),
            (Self::Matched | Self::Settled, _) => "The output settled.".into(),
            (Self::Deadline, Some(wanted)) => format!(
                "\"{wanted}\" did not show within {}s: the program may still be working; wait again or read the screen.",
                most.as_secs()
            ),
            (Self::Deadline, None) => "Still changing: wait again or read the screen.".into(),
            (Self::Exited, _) => "The program ended.".into(),
        }
    }
}
