//! The room for terminals: how many the desk holds at once (`terminal::room`, read from this
//! machine's open-file limit and memory as the desk starts), and the last terminal of each
//! project that could not open for want of it, which the Event Center lists quietly until a
//! terminal opens there again or the person dismisses it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// How many terminals the desk holds, and what it refused.
pub(in crate::desk) struct Room {
    most: AtomicUsize,
    refused: Mutex<HashMap<String, Refused>>,
}

/// A terminal that did not open: what it would have run, how many were open then, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::desk) struct Refused {
    /// Its number: a new refusal is news of its own, even one the person dismissed before.
    pub(in crate::desk) n: u64,
    /// What names it, as a terminal is named: its command, or `shell`.
    pub(in crate::desk) title: String,
    pub(in crate::desk) open: usize,
    pub(in crate::desk) at: u64,
}

/// The number of the last refusal.
static REFUSED: AtomicU64 = AtomicU64::new(0);

impl Default for Room {
    fn default() -> Self {
        Room {
            most: AtomicUsize::new(crate::terminal::room()),
            refused: Mutex::default(),
        }
    }
}

impl Room {
    /// The most terminals at once.
    pub(in crate::desk) fn most(&self) -> usize {
        self.most.load(Ordering::SeqCst)
    }

    /// A test's own bound, to reach it without opening a hundred programs.
    #[cfg(test)]
    pub(in crate::desk) fn hold(&self, most: usize) {
        self.most.store(most, Ordering::SeqCst);
    }

    /// A terminal of project `group` running `command` did not open with `open` terminals open:
    /// the words that say so, for whoever asked.
    pub(in crate::desk) fn refuse(&self, group: &str, command: &str, open: usize) -> String {
        let title = match command.trim() {
            "" => "shell",
            command => command,
        };
        let refused = Refused {
            n: REFUSED.fetch_add(1, Ordering::Relaxed) + 1,
            title: title.chars().take(TITLE_MOST).collect(),
            open,
            at: crate::board::now(),
        };
        crate::lock(&self.refused).insert(group.to_string(), refused);
        format!(
            "{}, the most this machine holds (by its memory and open-file limit), counting every \
             project's and those whose tab was closed with their program still running: close \
             one of yours with `close`, or ask the person to end one",
            are_open(open)
        )
    }

    /// A terminal of project `group` opened: whether a refusal there was news until now.
    pub(in crate::desk) fn opened(&self, group: &str) -> bool {
        crate::lock(&self.refused).remove(group).is_some()
    }

    /// The refusals that are news, by project.
    pub(in crate::desk) fn refused(&self) -> Vec<(String, Refused)> {
        let refused = crate::lock(&self.refused);
        let mut all: Vec<_> = (refused.iter())
            .map(|(group, refused)| (group.clone(), refused.clone()))
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }
}

/// The longest title kept, in characters: a terminal's command can be a script.
const TITLE_MOST: usize = 200;

/// `n` terminals are open, in words.
pub(in crate::desk) fn are_open(n: usize) -> String {
    match n {
        1 => "1 terminal is open".to_string(),
        n => format!("{n} terminals are open"),
    }
}
