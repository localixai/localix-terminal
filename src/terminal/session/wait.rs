//! Wait for screen text, quiet output, process exit or a shell's mark using the session's
//! condition variable.

use super::super::pty;
use super::{Session, Until};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// Output that stays still this long has settled.
const SETTLE: Duration = Duration::from_millis(400);
/// A wait with nothing named to wait for ends after this much: settled, if the output stopped, else
/// still changing (a program that prints nothing is quiet).
const QUIET: Duration = Duration::from_millis(1500);
/// A command at work that reads whole lines and stays quiet this long may be asking one: the
/// wait ends, and the screen says what it asks.
const ASKED: Duration = Duration::from_secs(3);
/// A command's end is told once the shell has run nothing since for this long.
const TAKEN: Duration = Duration::from_millis(40);
/// How often a quiet terminal is asked whether its program reads a line it does not show.
const ASK_EVERY: Duration = Duration::from_millis(250);
/// What the screen read says under it while the program does.
pub(super) const ASKING_NOTE: &str =
    "\n(the program is asking for hidden input: a password, most likely)";
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

    /// How many times `needle` showed before the last input was typed, with the input's own
    /// echo: a wait with no input of its own waits for the program's answer to it, which may
    /// have come before the wait began. Nothing typed yet: none.
    pub fn count_before_typed(&self, needle: &str) -> usize {
        if needle.is_empty() {
            return 0;
        }
        let state = self.shared.lock();
        (state.typed.as_ref()).map_or(0, |(screen, input)| {
            screen.matches(needle).count() + input.matches(needle).count()
        })
    }

    /// Wait for `needle` to show more than `before` times, for the output to settle, for the
    /// shell to say its command ended, or for the program to end; at most `most`.
    pub fn wait(&self, needle: Option<&str>, before: usize, most: Duration) -> Until {
        let needle = needle.filter(|needle| !needle.is_empty());
        let start = Instant::now();
        let mut state = self.shared.lock();
        let first = state.version;
        let ended = state.screen.marks().ended().0;
        let ended = state
            .typed_at
            .take()
            .map_or(ended, |typed| typed.min(ended));
        // The screen is looked in again only when output came in since.
        let mut looked = None;
        let mut asked: Option<Instant> = None;
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
            if let Some(last) = state.screen.marks().idle_since(ended, TAKEN) {
                return Until::Finished(last);
            }
            let idle = state.changed_at.elapsed();
            let moved = state.version != first;
            let running = state.screen.marks().running().is_some();
            // Quiet since before the call's own typing could have been read: how the terminal
            // takes typing tells a program that waits for the person from one at work.
            let mut typing = None;
            if idle >= SETTLE
                && start.elapsed() >= SETTLE
                && asked.is_none_or(|at| at.elapsed() >= ASK_EVERY)
            {
                asked = Some(Instant::now());
                typing = pty::Typing::of(&self.control);
                // A line it hides is said now, not when the time runs out.
                if typing.is_some_and(pty::Typing::hidden) {
                    return Until::Asking;
                }
            }
            if state.screen.marks().ended().0 > ended {
                // A command just ended: it is told once the shell is seen to run nothing more.
            } else if needle.is_none() && running {
                // A command at work is waited for to its end. One that takes each key is at
                // the person's hand (an editor, a prompt of its own); one that reads a line
                // may be asking a question, which a longer quiet tells.
                let quiet = match typing {
                    Some(typing) if typing.by_line => Some(ASKED),
                    Some(_) => Some(SETTLE),
                    None => None,
                };
                if quiet.is_some_and(|quiet| idle >= quiet && start.elapsed() >= quiet) {
                    return Until::Settled;
                }
            } else if needle.is_none() {
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
                return match (needle, running) {
                    (None, true) => Until::Running,
                    _ => Until::Deadline,
                };
            }
            let slice = left.min(Duration::from_millis(50));
            state = crate::wait_timeout(&self.shared.wake, state, slice);
        }
    }
}

impl Session {
    /// Wait for the shell to say its prompt is ready for a line (OSC 133 A), at most `most`:
    /// whether it did. A shell that marks nothing never does.
    pub fn wait_prompt(&self, most: Duration) -> bool {
        self.wait_prompt_after(0, most)
    }

    /// How many prompts the shell said it shows.
    pub fn prompts(&self) -> u64 {
        self.shared.lock().screen.marks().prompts()
    }

    /// Wait for the shell to say a prompt is ready once more than `after` were, at most `most`:
    /// whether it did.
    pub fn wait_prompt_after(&self, after: u64, most: Duration) -> bool {
        let deadline = Instant::now() + most;
        let mut state = self.shared.lock();
        loop {
            if state.screen.marks().prompts() > after {
                return true;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if state.ended || left.is_zero() {
                return false;
            }
            state = crate::wait_timeout(&self.shared.wake, state, left.min(WAKE_SLICE));
        }
    }

    /// How many commands the shell said ended.
    pub fn commands_ended(&self) -> u64 {
        self.shared.lock().screen.marks().ended().0
    }

    /// Wait for a command to end once more than `after` have (the shell's mark), at most
    /// `most`: how it ended; none when no command ended in time, or the shell itself did.
    pub fn wait_command_end(
        &self,
        after: u64,
        most: Duration,
    ) -> Option<super::super::marks::Ended> {
        let deadline = Instant::now() + most;
        let mut state = self.shared.lock();
        loop {
            if let (count, Some(ended)) = state.screen.marks().ended()
                && count > after
            {
                return Some(ended);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if state.ended || left.is_zero() {
                return None;
            }
            state = crate::wait_timeout(&self.shared.wake, state, left.min(WAKE_SLICE));
        }
    }
}

/// The longest a wait on a mark sleeps before it looks again: it is woken with output too.
const WAKE_SLICE: Duration = Duration::from_millis(250);

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
            (Self::Running, _) => format!(
                "The command is still running after {}s: wait again, or read the screen.",
                most.as_secs()
            ),
            (Self::Exited, _) => "The program ended.".into(),
            (Self::Finished(ended), wanted) => {
                let mut said = String::from("The command ended");
                if let Some(code) = ended.code {
                    let _ = write!(said, " with exit code {code}");
                }
                let _ = write!(said, " after {:.1}s", ended.took.as_secs_f64());
                match wanted {
                    Some(wanted) => format!("{said}; \"{wanted}\" did not show."),
                    None => format!("{said}."),
                }
            }
            (Self::Asking, _) => "The program is asking for hidden input (a password, most likely) and goes on only once it is typed: `ask` the person, or send a saved `{{secret:NAME}}`.".into(),
        }
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
