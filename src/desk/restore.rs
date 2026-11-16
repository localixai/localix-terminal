//! Reopen a project's panes from the ledger, preserving their rows and layout; a terminal's
//! coding agent goes on in it (agent_resume.rs keeps it, agent_relaunch.rs resumes it).

#[path = "agent_relaunch.rs"]
mod agent_relaunch;
#[path = "agent_resume.rs"]
pub(super) mod agent_resume;

use std::collections::{HashMap, HashSet};

use super::{Call, Desk};
use crate::ledger::{self, Kind, Row};
use crate::projects::ProjectId;

/// What this run knows of the tabs the run before left open.
#[derive(Debug, Default)]
pub(super) struct Past {
    /// The projects that have had a tab this run: what another run left open in them is past.
    pub(super) begun: HashSet<ProjectId>,
    /// The projects whose tabs are being opened again right now.
    pub(super) restoring: HashSet<ProjectId>,
    /// The coding agent each terminal of this run runs, by its key: its id and its session's,
    /// where known. A session live in one is never resumed in another.
    pub(super) agents: HashMap<String, (String, Option<String>)>,
    /// The agents of the run before going on in this run's terminals, or waiting for the
    /// person's word, by the terminal's key.
    pub(super) resumes: HashMap<String, Resume>,
}

/// A coding agent of the run before, going on in its terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Resume {
    pub(super) row: Row,
    /// The agent's name (`Claude Code`).
    pub(super) name: &'static str,
    /// What is typed into the terminal's shell: the command that resumes its session, or the
    /// one that continues its folder's last.
    pub(super) line: String,
    pub(super) session: Option<String>,
    pub(super) stage: Stage,
    /// How many times in a row it did not resume.
    pub(super) tries: u32,
}

/// How far a resume is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// Its command waits for the shell's prompt.
    Waiting,
    /// Its command was typed, and is watched for an early end.
    Typed,
    /// It did not resume twice: the person's Retry.
    Failed,
    /// No session to resume: the person may continue the folder's last.
    Continue,
}

impl Desk {
    /// Pane `row` of the history opened again for the person, as `call` (of the project the
    /// window keys `group`).
    pub(super) fn again(&self, call: &Call, group: &str, row: Row) -> Result<(), String> {
        let tab = (self.feed.store().read(|db| ledger::get(db, row)))?
            .ok_or("that tab is no longer in the history")?;
        self.again_tab(call, group, &tab, false)
    }

    /// The person went to `call`'s project, which has no tab open: what it had open when the
    /// app last quit opens again, the same panes in the same tabs, places and splits. Nothing
    /// left, nothing opens: the window offers its start of a new tab.
    pub(super) fn reopen_project(&self, call: &Call, group: &str) -> Result<(), String> {
        // Asked for twice in a row (the window as it opens, the person as they go there), it
        // runs once: the second would open what the first has not got to yet.
        if !crate::lock(&self.past).restoring.insert(call.project.id) {
            return Ok(());
        }
        let done = self.reopen_left(call, group);
        crate::lock(&self.past).restoring.remove(&call.project.id);
        done
    }

    /// `call`'s project opened again as the window does when the person goes to it: for tests,
    /// which have no person to go there.
    ///
    /// # Errors
    /// As the window's own reopening.
    #[cfg(feature = "test-server")]
    pub fn reopen_for(&self, call: &Call) -> Result<(), String> {
        self.reopen_project(call, &crate::projects::key(call.project.id))
    }

    fn reopen_left(&self, call: &Call, group: &str) -> Result<(), String> {
        let open: HashSet<Row> = (crate::lock(&self.ledger).values())
            .map(|(row, _)| *row)
            .collect();
        let project = call.project.id;
        let (left, order) = (self.feed.store()).read(|db| {
            let left = ledger::left_open(db, project)?;
            Ok((left, Self::panes_in_order(db, project)?))
        })?;
        let mut left: Vec<ledger::Pane> = (left.into_iter())
            .filter(|tab| !open.contains(&tab.row))
            .collect();
        if left.is_empty() {
            return Ok(());
        }
        left.sort_by_key(|tab| order.iter().position(|row| *row == tab.row));
        // They are this run's now: nothing of the project's closes as the first of them opens.
        crate::lock(&self.past).begun.insert(project);
        // A file has one tab in its project: a second row of the same file opens nothing.
        let mut files = HashSet::new();
        for tab in &left {
            let twice = tab.kind == Kind::File && !files.insert(tab.cwd.clone());
            let opened = match twice {
                true => Err("its file has a tab already".to_string()),
                false => self.again_tab(call, group, tab, true),
            };
            if let Err(why) = opened {
                log::warn!("a tab of {} did not open again: {why}", call.project.name);
                let now = crate::board::now();
                let closed = self.feed.write(|write| {
                    ledger::close(write.tx(), tab.row, now)?;
                    let ledger = crate::feed::Entity::Ledger { project };
                    write.record(&ledger, crate::feed::Op::Upserted)?;
                    Self::unplace_pane(write, tab.row, now)
                });
                if let Err(why) = closed {
                    log::warn!("it stays open in the ledger: {why}");
                }
            }
        }
        Ok(())
    }

    /// The pane `tab` opened again for the person: a page at its address, their shell in a terminal's
    /// folder (its program is not run again, but the coding agent it ran goes on, `resume`
    /// as the app opens), the board, a file in its editor. It is the same pane as before: its row,
    /// its history, its place in its tab.
    fn again_tab(
        &self,
        call: &Call,
        group: &str,
        tab: &ledger::Pane,
        resume: bool,
    ) -> Result<(), String> {
        let call = &Call {
            again: Some(tab.row),
            ..*call
        };
        match tab.kind {
            Kind::Page => {
                let url = (tab.url.as_deref())
                    .filter(|url| !url.is_empty())
                    .unwrap_or(crate::browser::HOME);
                (self.open(call, &serde_json::json!({"kind": "browser", "url": url})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string())
            }
            Kind::Terminal => {
                let plan = resume.then(|| self.resume_plan(call, tab.row)).flatten();
                let folder = plan.as_ref().and_then(|plan| plan.folder.clone());
                let cwd = (folder.into_iter().chain(tab.cwd.clone()))
                    .find(|cwd| cwd.is_dir())
                    .unwrap_or_else(|| call.cwd.to_path_buf());
                let mut before = (self.feed.store().read(|db| ledger::screen(db, tab.row)))
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if let Some(said) = plan.as_ref().and_then(|plan| plan.said.as_deref()) {
                    before = match before.trim_end() {
                        "" => format!("── {said}"),
                        kept => format!("{kept}\n── {said}"),
                    };
                }
                let call = &Call { cwd: &cwd, ..*call };
                let key = self.terminal_of_person(call, group, "", "", &before)?;
                if let Some(plan) = plan {
                    self.resume_in(&key, tab.row, plan);
                }
                Ok(())
            }
            Kind::Board => (self.board(call, &serde_json::json!({"op": "show"})))
                .map(|_| ())
                .map_err(|failure| failure.message().to_string()),
            Kind::Made(name) => {
                let about = (self.made.kinds.iter().map(|kind| kind.about()))
                    .find(|about| about.id == name)
                    .ok_or_else(|| format!("this build has no kind of tab `{name}`"))?;
                (self.made(call, about, &serde_json::json!({})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string())
            }
            Kind::File => {
                let path = tab.cwd.as_deref().ok_or("its file was not kept")?;
                (self.open_file(call, path, None))
                    .map(drop)
                    .map_err(|not| not.why)
            }
        }
    }
}
