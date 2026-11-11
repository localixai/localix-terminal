//! Where an event leads: what the person is taken to when they click it, in the list or on its
//! system notice. A source names the thing itself (a tab, a board, an agent's session), never
//! how it is reached; here, once, the thing is found in the window or opened in it. A new kind
//! of thing is a variant and its arm in `lead`, and every source may then lead to it.

use std::path::PathBuf;
use std::sync::Arc;

use crate::browser::Asked;
use crate::desk::Desk;

/// What an event leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::desk::center) enum Leads {
    /// A tab that is open: in front.
    Tab(String),
    /// The board of the project grouped so: in front, opened again if it was closed.
    Board(String),
    /// A coding agent's session that runs outside the window, so it has no tab to show:
    /// continued by its `program` in a terminal of the project grouped as `group`, in the
    /// `folder` its program works in. `owner` is whose wait it answers.
    Session {
        owner: String,
        group: String,
        program: String,
        folder: PathBuf,
    },
}

impl Desk {
    /// The person is taken where `leads` says.
    pub(in crate::desk::center) fn lead(self: &Arc<Self>, leads: Leads) {
        match leads {
            Leads::Tab(key) => {
                if let Some(browser) = &self.browser {
                    let _ = browser.reveal(&key);
                }
            }
            Leads::Board(group) => self.person_asked(&group, Asked::Board),
            Leads::Session {
                owner,
                group,
                program,
                folder,
            } => match crate::browser::agent_continued(&program) {
                Some(again) => {
                    let asked = Asked::Session {
                        command: again.line,
                        title: again.name.to_string(),
                        folder,
                        waits: owner,
                    };
                    self.person_asked(&group, asked);
                }
                None => log::warn!("desk: `{program}` is no agent that continues a session"),
            },
        }
    }
}

impl Desk {
    /// The session of `owner` that waited outside the window was continued in a terminal of
    /// it: its line is done with, since it is a tab from here on. Only once that terminal
    /// opened: a line that led nowhere stays.
    pub(in crate::desk) fn continued_here(&self, owner: &str) {
        self.ask(owner, super::Subject::Session(owner.to_string()), None);
    }
}

/// The coding agent's program process `pid` is, or descends from (the nearest), and the
/// folder that program works in.
pub fn agent_of(pid: libc::pid_t) -> Option<(String, Option<PathBuf>)> {
    crate::terminal::lineage::program_among(pid, crate::browser::agent_runs)
}

#[cfg(test)]
#[path = "leads_tests.rs"]
mod tests;
