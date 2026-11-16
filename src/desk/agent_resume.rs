//! A terminal's coding agent, kept as it is seen (docs/windows.md, "An agent goes on"): each
//! time another program comes in front, its command line is read through the agents' lists
//! (`browser::agent_launch`) and kept for the terminal's pane at once, so a crash keeps it as a
//! quit does; the session its own hooks name is kept with it (`interact` with `session` and no
//! tab, from the terminal it runs in), written then and there. An agent that ends of itself, or
//! a run that prints and ends, is let go of; while the app quits, its agents end with it and are
//! kept. An agent seen in front of a terminal whose agent of the run before waits for the
//! person's word takes its place. A board's worker is never kept: it would wake with no task.

use serde_json::{Value, json};

use super::Stage;
use crate::desk::{Call, Desk};
use crate::ledger::{self, AgentSession};

/// `interact`'s field for an agent's own hooks.
pub(in crate::desk) fn session_field() -> Value {
    json!({"type": "string", "description": "interact, no tab: the id of the coding agent's \
        session this terminal runs, as its own hooks know it, kept so the session goes on here \
        after the app opens again."})
}

impl Desk {
    /// The program in front of terminal `id` changed: its agent kept as it is now, or let go of.
    pub(in crate::desk) fn agent_fronted(&self, id: u32) {
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let key = format!("t{id}");
        let Some((row, _)) = crate::lock(&self.ledger).get(&key).copied() else {
            return;
        };
        if crate::lock(&self.workers).contains_key(&key) {
            return;
        }
        let Ok(session) = self.session(id) else {
            return;
        };
        let launch = (session.front_command()).and_then(|argv| crate::browser::agent_launch(&argv));
        // A value that names a saved secret is never kept, whatever the lists allow.
        let launch = launch.filter(|launch| {
            let words = launch.kept.join(" ");
            self.vault.mask(&words) == words
        });
        let kept = launch.map(|launch| AgentSession {
            pane: row,
            agent: launch.agent.to_string(),
            session: launch.session,
            argv: launch.kept,
            folder: session.folder(),
            bypass: launch.bypass,
            tries: 0,
        });
        // An agent ended only where one was seen in this run, and not while its command is
        // being typed or watched: the shell in front then is the moment before its command, or
        // after one that did not resume.
        let (ended, dropped) = {
            let mut past = crate::lock(&self.past);
            let stage = past.resumes.get(&key).map(|resume| resume.stage);
            let resuming = matches!(stage, Some(Stage::Waiting | Stage::Typed));
            match &kept {
                Some(kept) => {
                    let had = past.agents.get(&key).and_then(|(agent, session)| {
                        (*agent == kept.agent).then(|| session.clone()).flatten()
                    });
                    let session = kept.session.clone().or(had);
                    past.agents
                        .insert(key.clone(), (kept.agent.clone(), session));
                    // An agent in front of a terminal whose resume waits for the person's word
                    // (or was not typed yet): theirs runs, and the word is not wanted.
                    let pending = stage.is_some_and(|stage| stage != Stage::Typed);
                    let dropped = pending && past.resumes.remove(&key).is_some();
                    (false, dropped)
                }
                None => (past.agents.remove(&key).is_some() && !resuming, false),
            }
        };
        if dropped {
            self.feed.record(
                &crate::feed::Entity::Terminal { id },
                crate::feed::Op::Upserted,
            );
        }
        if kept.is_none() && !ended {
            return;
        }
        // Written now, not on the writer thread: a crash keeps what was seen, as a quit does.
        let wrote = self.feed.store().write(|tx| match &kept {
            Some(kept) => ledger::keep_agent(tx, kept, crate::board::now()),
            None => ledger::agent_ended(tx, row),
        });
        if let Err(why) = wrote {
            log::warn!("terminal t{id}'s agent is not kept: {why}");
        }
    }

    /// Terminal `key` runs a worker of the board: what was kept of its agent goes.
    pub(in crate::desk) fn agent_let_go(&self, key: &str) {
        crate::lock(&self.past).agents.remove(key);
        let Some((row, _)) = crate::lock(&self.ledger).get(key).copied() else {
            return;
        };
        if let Err(why) = self.feed.store().write(|tx| ledger::agent_ended(tx, row)) {
            log::warn!("a worker's agent stays kept: {why}");
        }
    }

    /// `session`, the id the agent in `call`'s terminal gives its session, kept for it.
    pub(in crate::desk) fn agent_said(&self, call: &Call, session: &str) -> Result<String, String> {
        let key = (call.from)
            .filter(|key| key.starts_with('t'))
            .ok_or("`session` is said from a terminal of the window, by the agent running in it")?;
        let id = key.get(1..).and_then(|id| id.parse::<u32>().ok());
        let found = crate::lock(&self.ledger).get(key).copied();
        let (Some(id), Some((row, _))) = (id, found) else {
            return Err(format!("there is no terminal {key}"));
        };
        let front = (self.session(id)?.front_command())
            .and_then(|argv| crate::browser::agent_launch(&argv));
        let agent = front
            .ok_or_else(|| format!("no coding agent runs in {key}"))?
            .agent;
        if !crate::browser::agent_names_session(agent, session) {
            return Err(format!("`{session}` is no session id of {agent}'s"));
        }
        if crate::lock(&self.workers).contains_key(key) {
            return Ok(format!(
                "{key} runs a worker of the board: its session is not kept."
            ));
        }
        // Kept as it is now first, so a session said before its agent was seen is kept too.
        self.agent_fronted(id);
        if let Some((_, said)) = crate::lock(&self.past).agents.get_mut(key) {
            *said = Some(session.to_string());
        }
        let kept = (self.feed.store()).write(|tx| ledger::agent_named(tx, row, agent, session));
        if let Err(why) = kept {
            log::warn!("terminal {key}'s session is not kept: {why}");
        }
        Ok(format!(
            "Kept: {key} goes on in this session after the app opens again."
        ))
    }
}

#[cfg(test)]
#[path = "agent_resume_tests.rs"]
pub(in crate::desk) mod tests;
