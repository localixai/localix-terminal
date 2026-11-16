//! What a shell says of itself between its output: that its prompt is ready, when a command
//! starts and how it ended (OSC 133). The words are a program's, untrusted as its output is: they are counted and told
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
    /// How many prompts it said it shows (`A`): its first, that it is ready for a line.
    prompts: u64,
    /// Since when a command runs, and whether the person's own keys typed it.
    running: Option<(Instant, bool)>,
    ended: u64,
    last: Option<(Ended, Instant, bool)>,
    /// Something was typed since the shell last said its prompt is ready: its line may hold it.
    typed: bool,
    /// The person's own keys were among it (an agent's `type_in` is not theirs).
    person: bool,
}

impl Marks {
    /// Reads a string sequence's words (`133;D;0`). An end counts only for a command whose
    /// start was said: a shell that says one at every prompt ran nothing on an empty line, and
    /// one whose marks are said twice ran each command once.
    pub(super) fn read(&mut self, said: &str) {
        let mut words = said.split(';');
        match (words.next(), words.next()) {
            (Some("133"), Some("A")) => {
                self.prompts += 1;
                self.typed = false;
                self.person = false;
            }
            (Some("133"), Some("C")) => {
                let person = self.person;
                self.running.get_or_insert_with(|| (Instant::now(), person));
            }
            (Some("133"), Some("D")) => {
                if let Some((since, theirs)) = self.running.take() {
                    let code = words.next().and_then(|code| code.parse().ok());
                    let took = since.elapsed();
                    self.last = Some((Ended { code, took }, Instant::now(), theirs));
                    self.ended += 1;
                }
            }
            _ => {}
        }
    }

    /// How many prompts the shell said it shows.
    pub(super) fn prompts(&self) -> u64 {
        self.prompts
    }

    /// Something was typed into the terminal: by the person's own keys, or for them (an agent's
    /// `type_in`).
    pub(super) fn typed_in(&mut self, person: bool) {
        self.typed = true;
        self.person |= person;
    }

    /// Whether something was typed since the shell last said its prompt is ready.
    pub(super) fn typed_since_prompt(&self) -> bool {
        self.typed
    }

    /// Since when a command runs, while one does.
    pub(super) fn running(&self) -> Option<Instant> {
        self.running.map(|(since, _)| since)
    }

    /// How many commands ended, and how the last did.
    pub(super) fn ended(&self) -> (u64, Option<Ended>) {
        (self.ended, self.last.map(|(ended, ..)| ended))
    }

    /// Whether the last command that ended was the person's: their own keys went in between the
    /// prompt before it and its start.
    pub(super) fn theirs(&self) -> bool {
        self.last.is_some_and(|(.., theirs)| theirs)
    }

    /// The last command's end, if more than `before` have ended and the shell has run nothing
    /// since for `quiet`: a line typed while a command ran is taken the moment that one ends,
    /// and the end then waited for is its own.
    pub(super) fn idle_since(&self, before: u64, quiet: Duration) -> Option<Ended> {
        let (ended, at, _) = self.last?;
        (self.ended > before && self.running.is_none() && at.elapsed() >= quiet).then_some(ended)
    }
}

#[cfg(test)]
#[path = "marks_tests.rs"]
mod tests;
