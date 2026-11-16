//! A coding agent of the run before goes on in its terminal as the app opens again
//! (docs/windows.md, "An agent goes on"): the terminal opens as the person's shell with its last
//! screen, a line under it says the agent's session was resumed, and once the shell says its
//! prompt is ready the command that resumes the session is typed into it, as the person would
//! type it: only into the shell in front, at a prompt nothing was typed at, with nothing at
//! work. A command that ends at once with an error is typed once more, then waits for the
//! person's Retry in the Event Center, and one that ends well at once is let go of; an agent
//! whose session is not known waits there for the person's Continue, which goes on with its
//! folder's last session. Nothing is resumed while the person turned it off in Settings, and a
//! session live in another terminal is never resumed.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::{Resume, Stage};
use crate::desk::{Call, Desk};
use crate::ledger::{self, AgentSession, Row};

/// How long a terminal's shell may take to show its first prompt before nothing is typed.
const READY: Duration = Duration::from_secs(30);
/// A command that ends with an error within this long after it was typed did not resume.
const WATCHED: Duration = Duration::from_secs(10);
/// How many times in a row a session is typed before it waits for the person.
const TRIES: u32 = 2;

/// What is done with a terminal's agent as the app opens again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::desk) struct Plan {
    pub(in crate::desk) name: &'static str,
    pub(in crate::desk) line: String,
    pub(in crate::desk) session: Option<String>,
    /// The line shown above the shell, when the command is to be typed.
    pub(in crate::desk) said: Option<String>,
    /// The folder the agent worked in.
    pub(in crate::desk) folder: Option<PathBuf>,
    pub(in crate::desk) stage: Stage,
    pub(in crate::desk) tries: u32,
}

/// What is done with `kept`, an agent the run before left running: `on`, resuming is on in
/// Settings; `live`, its session runs in another terminal already; `path`, where its command is
/// looked for. None: nothing, not even a line.
pub(in crate::desk) fn decide(
    kept: &AgentSession,
    on: bool,
    live: bool,
    path: &OsStr,
) -> Option<Plan> {
    if !on || live {
        return None;
    }
    let resumed = (kept.session.as_deref())
        .map(|session| crate::browser::agent_resumed(&kept.agent, session, &kept.argv, path));
    let plan = |name, line, stage, said| Plan {
        name,
        line,
        session: kept.session.clone(),
        said,
        folder: kept.folder.clone(),
        stage,
        tries: kept.tries,
    };
    match resumed {
        Some(Ok(resumed)) if kept.tries < TRIES => {
            let said = resumed.said(kept.bypass);
            Some(plan(resumed.name, resumed.line, Stage::Waiting, Some(said)))
        }
        Some(Ok(resumed)) => Some(plan(resumed.name, resumed.line, Stage::Failed, None)),
        // No session known, or none to resume by its id here: the folder's last, by the
        // person's word.
        _ => {
            let continued = crate::browser::agent_continued(&kept.agent)?;
            Some(plan(continued.name, continued.line, Stage::Continue, None))
        }
    }
}

impl Desk {
    /// What is done with the agent terminal pane `row` ran when the app last quit, as `call`
    /// opens it again; none where it ran none. One skipped for good (resuming is off, its
    /// session runs in another terminal, nothing continues it) is let go of, never read again.
    pub(in crate::desk) fn resume_plan(&self, call: &Call, row: Row) -> Option<Plan> {
        let kept = (self.feed.store().read(|db| ledger::agent_of(db, row))).ok()??;
        // Where the shell will look for it: the caller's `PATH`, else the person's.
        let path = (call
            .vars
            .and_then(|vars| vars.iter().find(|(name, _)| name == "PATH")))
        .map(|(_, path)| OsString::from(path))
        .or_else(crate::browser::commands_path)
        .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
        let past = crate::lock(&self.past);
        let same = |agent: &str, session: Option<&str>| {
            agent == kept.agent && session.is_some() && session == kept.session.as_deref()
        };
        let live = (past.agents.values()).any(|(agent, session)| same(agent, session.as_deref()))
            || (past.resumes.values())
                .any(|resume| resume.session.is_some() && resume.session == kept.session);
        drop(past);
        let plan = decide(&kept, crate::browser::resumes_agents(), live, &path);
        if plan.is_none()
            && let Err(why) = (self.feed.store()).write(|tx| ledger::agent_ended(tx, row))
        {
            log::warn!("an agent not resumed stays kept: {why}");
        }
        plan
    }

    /// Terminal `key` (pane `row`) opened again goes on as `plan` says: its command typed once
    /// its shell is ready, or a line in the Event Center.
    pub(in crate::desk) fn resume_in(&self, key: &str, row: Row, plan: Plan) {
        let resume = Resume {
            row,
            name: plan.name,
            line: plan.line,
            session: plan.session,
            stage: plan.stage,
            tries: plan.tries,
        };
        let typed = resume.stage == Stage::Waiting;
        crate::lock(&self.past)
            .resumes
            .insert(key.to_string(), resume);
        match typed {
            true => self.resume_later(key, TRIES, false),
            false => self.resumes_changed(key),
        }
    }

    /// The person's Retry, or Continue, on terminal `key`'s line: its command typed once more,
    /// and the terminal in front. Only a line that waits for them is taken, and taken at once,
    /// so a second click while the first goes on starts nothing.
    pub(in crate::desk) fn resume_again(&self, key: &str) {
        {
            let mut past = crate::lock(&self.past);
            let Some(resume) = past.resumes.get_mut(key) else {
                return;
            };
            if !matches!(resume.stage, Stage::Failed | Stage::Continue) {
                return;
            }
            resume.stage = Stage::Waiting;
        }
        if let Some(browser) = &self.browser {
            let _ = browser.reveal(key);
        }
        self.resume_later(key, 1, true);
        self.resumes_changed(key);
    }

    /// Terminal `key`'s command typed on a thread of its own, at most `times` times (`asked`:
    /// by the person's click).
    fn resume_later(&self, key: &str, times: u32, asked: bool) {
        let (me, typed) = (std::sync::Weak::clone(&self.me), key.to_string());
        let started = std::thread::Builder::new()
            .name("desk-agent-resume".into())
            .spawn(move || {
                if let Some(desk) = me.upgrade() {
                    desk.resume_typed(&typed, times, asked);
                }
            });
        if let Err(why) = started {
            log::warn!("terminal {key}'s agent is not resumed (no thread: {why})");
        }
    }

    /// Terminal `key`'s command typed once its shell is ready, and watched: a command that ends
    /// with an error at once is typed again, up to `times`; then the person's word is waited for.
    /// Unasked, a shell must say its prompt is ready first; the person's click needs no mark,
    /// only the shell in front with nothing at work (a shell that marks nothing: fish, tmux).
    fn resume_typed(self: &Arc<Self>, key: &str, times: u32, asked: bool) {
        let id = key.get(1..).and_then(|id| id.parse::<u32>().ok());
        let Some(session) = id.and_then(|id| self.session(id).ok()) else {
            return;
        };
        // A shell that never says its prompt is ready is typed nothing unasked: its line says
        // the command.
        if !asked && !session.wait_prompt(READY) {
            self.resume_staged(key, Stage::Continue, None);
            return;
        }
        for _ in 0..times {
            let prompts = session.prompts();
            let Some(line) = self.resume_turn(key, &session, asked) else {
                return;
            };
            let before = session.commands_ended();
            if session.type_in(&format!("{line}\n"), &[]).is_err() {
                return;
            }
            match session.wait_command_end(before, WATCHED) {
                // It runs: it went on.
                None => return self.resume_done(key, false),
                // It ended well at once, or the person ended it: nothing failed, nothing to resume.
                Some(ended) if ended.code.is_none_or(|code| code == 0) => {
                    return self.resume_done(key, true);
                }
                Some(_) => {}
            }
            let tries = crate::lock(&self.past)
                .resumes
                .get(key)
                .map_or(1, |r| r.tries + 1);
            self.resume_staged(key, Stage::Failed, Some(tries));
            // The next try is typed at the prompt the shell shows after this one's end.
            if tries >= TRIES || !session.wait_prompt_after(prompts, READY) {
                break;
            }
            self.resume_staged(key, Stage::Waiting, None);
        }
        self.resume_staged(key, Stage::Failed, None);
    }

    /// Terminal `key`'s line, to be typed now: only while its resume waits to be typed, and only
    /// into its shell at a prompt nothing was typed at, with nothing at work (`asked`: the
    /// person's click, which needs no prompt mark). Else nothing is typed, and the line waits
    /// for the person's Continue.
    fn resume_turn(
        &self,
        key: &str,
        session: &crate::terminal::Session,
        asked: bool,
    ) -> Option<String> {
        let clean = session.at_prompt(!asked);
        let line = {
            let mut past = crate::lock(&self.past);
            let resume =
                (past.resumes.get_mut(key)).filter(|resume| resume.stage == Stage::Waiting)?;
            resume.stage = if clean { Stage::Typed } else { Stage::Continue };
            clean.then(|| resume.line.clone())
        };
        self.resumes_changed(key);
        line
    }

    /// Terminal `key`'s agent went on, or `ended` well at once: its resume let go of, and its
    /// row with it where it ended, else its tries counted afresh.
    fn resume_done(&self, key: &str, ended: bool) {
        let row = crate::lock(&self.past)
            .resumes
            .remove(key)
            .map(|resume| resume.row);
        if let Some(row) = row {
            let kept = (self.feed.store()).write(|tx| match ended {
                true => ledger::agent_ended(tx, row),
                false => ledger::agent_tried(tx, row, 0),
            });
            if let Err(why) = kept {
                log::warn!("terminal {key}'s agent is not set right: {why}");
            }
        }
        self.resumes_changed(key);
    }

    /// Terminal `key`'s resume is at `stage`, `tries` the count kept in the store when it changed.
    fn resume_staged(&self, key: &str, stage: Stage, tries: Option<u32>) {
        let row = {
            let mut past = crate::lock(&self.past);
            let Some(resume) = past.resumes.get_mut(key) else {
                return;
            };
            resume.stage = stage;
            if let Some(tries) = tries {
                resume.tries = tries;
            }
            resume.row
        };
        if let Some(tries) = tries {
            let kept = (self.feed.store()).write(|tx| ledger::agent_tried(tx, row, tries));
            if let Err(why) = kept {
                log::warn!("terminal {key}'s tries are not kept: {why}");
            }
        }
        self.resumes_changed(key);
    }

    /// What terminal `key`'s resume says changed: news for the Event Center.
    fn resumes_changed(&self, key: &str) {
        if let Some(id) = key.get(1..).and_then(|id| id.parse::<u32>().ok()) {
            let terminal = crate::feed::Entity::Terminal { id };
            self.feed.record(&terminal, crate::feed::Op::Upserted);
        }
    }
}

#[cfg(test)]
#[path = "agent_relaunch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "agent_relaunch_asked_tests.rs"]
mod asked_tests;
