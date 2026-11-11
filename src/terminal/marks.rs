//! What a shell says of itself between its output: when a command starts and how it ended
//! (OSC 133). The words are a program's, untrusted as its output is: they are counted and told
//! on, never acted on.

use std::time::{Duration, Instant};

/// How a command ended: its exit code when the shell said one, and how long it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    pub code: Option<i32>,
    pub took: Duration,
}

#[derive(Debug, Default)]
pub(super) struct Marks {
    running: Option<Instant>,
    ended: u64,
    last: Option<(Ended, Instant)>,
}

impl Marks {
    /// Reads a string sequence's words (`133;D;0`). An end counts only for a command whose
    /// start was said: a shell that says one at every prompt ran nothing on an empty line, and
    /// one whose marks are said twice ran each command once.
    pub(super) fn read(&mut self, said: &str) {
        let mut words = said.split(';');
        match (words.next(), words.next()) {
            (Some("133"), Some("C")) => {
                self.running.get_or_insert_with(Instant::now);
            }
            (Some("133"), Some("D")) => {
                if let Some(since) = self.running.take() {
                    let code = words.next().and_then(|code| code.parse().ok());
                    let took = since.elapsed();
                    self.last = Some((Ended { code, took }, Instant::now()));
                    self.ended += 1;
                }
            }
            _ => {}
        }
    }

    /// Since when a command runs, while one does.
    pub(super) fn running(&self) -> Option<Instant> {
        self.running
    }

    /// How many commands ended, and how the last did.
    pub(super) fn ended(&self) -> (u64, Option<Ended>) {
        (self.ended, self.last.map(|(ended, _)| ended))
    }

    /// The last command's end, if more than `before` have ended and the shell has run nothing
    /// since for `quiet`: a line typed while a command ran is taken the moment that one ends,
    /// and the end then waited for is its own.
    pub(super) fn idle_since(&self, before: u64, quiet: Duration) -> Option<Ended> {
        let (ended, at) = self.last?;
        (self.ended > before && self.running.is_none() && at.elapsed() >= quiet).then_some(ended)
    }
}

#[cfg(test)]
#[path = "marks_tests.rs"]
mod tests;
