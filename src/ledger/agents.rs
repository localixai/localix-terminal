//! The coding agent a terminal pane runs, as the ledger keeps it: written as the agent is seen
//! in front (and as its hooks tell its session), so the app opens it again after a quit or a
//! crash alike; gone when the agent ends of itself or the person closes its terminal. What is
//! kept is words, never a command: the command is built again from the agents' table.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Row, failed};

/// A terminal's agent as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSession {
    pub pane: Row,
    /// The agent's id in the agents' table (`claude`).
    pub agent: String,
    /// Its session's id, where it is known.
    pub session: Option<String>,
    /// The words of its command line that are carried, already read through its list.
    pub argv: Vec<String>,
    /// The folder it works in.
    pub folder: Option<PathBuf>,
    /// A sandbox bypass its line had was dropped.
    pub bypass: bool,
    /// How many times in a row it did not resume.
    pub tries: u32,
}

/// `agent` is seen running in its pane, `now`: kept, its session's id with it where it is
/// known. One whose id is not known keeps the id its hooks told for the same agent; another
/// agent or session counts its tries afresh.
///
/// # Errors
/// The store refused it.
pub fn keep_agent(tx: &Transaction<'_>, agent: &AgentSession, now: u64) -> Result<(), String> {
    let argv = serde_json::to_string(&agent.argv).map_err(|why| why.to_string())?;
    let folder = agent
        .folder
        .as_ref()
        .map(|folder| folder.display().to_string());
    tx.execute(
        "INSERT INTO agent_sessions (pane, agent, session, argv, folder, bypass, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (pane) DO UPDATE SET
             session = COALESCE(excluded.session,
                 CASE WHEN agent = excluded.agent THEN session END),
             -- What another agent or session did not do is not this one's: counted afresh.
             tries = CASE WHEN agent = excluded.agent
                 AND (excluded.session IS NULL OR excluded.session IS session)
                 THEN tries ELSE 0 END,
             agent = excluded.agent, argv = excluded.argv, folder = excluded.folder,
             bypass = excluded.bypass, at = excluded.at",
        params![
            agent.pane,
            agent.agent,
            agent.session,
            argv,
            folder,
            agent.bypass,
            now
        ],
    )
    .map(drop)
    .map_err(|why| failed(&why))
}

/// The agent `agent` of pane `pane` named its session `session` (its own hooks said so):
/// whether that was its agent.
///
/// # Errors
/// The store refused it.
pub fn agent_named(
    tx: &Transaction<'_>,
    pane: Row,
    agent: &str,
    session: &str,
) -> Result<bool, String> {
    let set = "UPDATE agent_sessions SET session = ?3,
                   tries = CASE WHEN session IS ?3 THEN tries ELSE 0 END
               WHERE pane = ?1 AND agent = ?2";
    (tx.execute(set, params![pane, agent, session]))
        .map(|changed| changed > 0)
        .map_err(|why| failed(&why))
}

/// Pane `pane`'s agent did not resume `tries` times in a row (0: it did).
///
/// # Errors
/// The store refused it.
pub fn agent_tried(tx: &Transaction<'_>, pane: Row, tries: u32) -> Result<(), String> {
    let set = "UPDATE agent_sessions SET tries = ?2 WHERE pane = ?1";
    (tx.execute(set, params![pane, tries]))
        .map(drop)
        .map_err(|why| failed(&why))
}

/// Pane `pane`'s agent ended, or its terminal closed: nothing of it is resumed.
///
/// # Errors
/// The store refused it.
pub fn agent_ended(tx: &Transaction<'_>, pane: Row) -> Result<(), String> {
    (tx.execute("DELETE FROM agent_sessions WHERE pane = ?1", [pane]))
        .map(drop)
        .map_err(|why| failed(&why))
}

/// Pane `pane`'s agent, while one is kept. Words a row holds that do not read are none.
///
/// # Errors
/// The store could not be read.
pub fn agent_of(db: &Connection, pane: Row) -> Result<Option<AgentSession>, String> {
    db.query_row(
        "SELECT agent, session, argv, folder, bypass, tries FROM agent_sessions WHERE pane = ?1",
        [pane],
        |row| {
            let argv: String = row.get(2)?;
            let folder: Option<String> = row.get(3)?;
            Ok(AgentSession {
                pane,
                agent: row.get(0)?,
                session: row.get(1)?,
                argv: serde_json::from_str(&argv).unwrap_or_default(),
                folder: folder.map(PathBuf::from),
                bypass: row.get(4)?,
                tries: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|why| failed(&why))
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
