//! The workers the person ran from a board. Each runs in a terminal of its own, and a session in
//! that terminal is that worker: an agent of the project by its name on the board, with no word
//! from the session itself. What the person says of its card is told to it alone while its
//! terminal runs, and to the project's agent once it is over.

use crate::projects::ProjectId;

use super::{Call, Desk};

/// A worker in its terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Worker {
    project: ProjectId,
    /// Its name on the board.
    who: String,
}

impl Desk {
    /// The person's terminal for worker `who` of `call`'s project, running `command`: its key.
    pub(super) fn worker_terminal(
        &self,
        call: &Call,
        group: &str,
        command: &str,
        title: &str,
        who: &str,
    ) -> Result<String, String> {
        let key = self.terminal_of_person(call, group, command, title, "")?;
        let worker = Worker {
            project: call.project.id,
            who: who.to_string(),
        };
        let mut workers = crate::lock(&self.workers);
        // A terminal that is over runs no worker: its name is free for the card's next run.
        workers.retain(|key, _| self.runs(key));
        workers.insert(key.clone(), worker);
        Ok(key)
    }

    /// The worker of `project` that runs in terminal `key`: who a caller from it is.
    pub(super) fn worker_in(&self, key: Option<&str>, project: ProjectId) -> Option<String> {
        let workers = crate::lock(&self.workers);
        let worker = workers
            .get(key?)
            .filter(|worker| worker.project == project)?;
        Some(worker.who.clone())
    }

    /// The worker of `project` the process `pid` descends from, when the caller names no agent
    /// itself. Nothing is looked up while no worker runs.
    pub fn worker_at(&self, pid: libc::pid_t, project: ProjectId) -> Option<String> {
        if crate::lock(&self.workers).is_empty() {
            return None;
        }
        self.worker_in(self.pane_of(pid).as_deref(), project)
    }

    /// The owner that hears what is said of worker `who`'s card in `project`: the worker, while
    /// its terminal runs.
    pub(super) fn worker_owner(&self, project: ProjectId, who: &str) -> Option<String> {
        let workers = crate::lock(&self.workers);
        (workers.iter())
            .find(|(key, worker)| worker.project == project && worker.who == who && self.runs(key))
            .map(|_| format!("{project}/{who}"))
    }

    /// Terminal `key`'s program still runs.
    fn runs(&self, key: &str) -> bool {
        let id = key.strip_prefix('t').and_then(|id| id.parse::<u32>().ok());
        id.and_then(|id| crate::lock(&self.sessions).get(&id).cloned())
            .is_some_and(|session| !session.ended())
    }
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
