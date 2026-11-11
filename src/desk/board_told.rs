//! What the person does on a board, and how the agent hears it. An action is applied and painted
//! at once; the agent is told once the person has been quiet a moment, everything they did in one
//! note, so a few cards dragged in a row are one message, not five. A note with something worth
//! acting on now (a card sent back, a comment) is told to whoever subscribed, as a comment is; one
//! with only quiet news (a card accepted, reordered) waits for the agent's next call.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Weak;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::Duration;

use serde_json::Value;

use crate::board::{self, Did, Urgency};
use crate::browser::Asked;

use crate::projects::ProjectId;

use super::Desk;

/// How long the person is quiet before the agent is told what they did.
#[cfg(not(test))]
const QUIET: Duration = Duration::from_millis(1200);
#[cfg(test)]
const QUIET: Duration = Duration::from_millis(50);

/// Owners whose boards have news, waiting for the thread that tells them.
const WAITING: usize = 64;

impl Desk {
    /// The person's `posted` action on project `project`'s board, as its page sends it: applied, kept and
    /// painted; refused with why, painted as it was.
    pub(super) fn person_on_board(&self, project: ProjectId, posted: &str) {
        let action: Result<Value, String> =
            serde_json::from_str(posted).map_err(|why| format!("the board sent no action: {why}"));
        let mut started = None;
        let paint = {
            let mut boards = crate::lock(&self.boards);
            let Some(kept) = boards.get_mut(&project) else {
                return;
            };
            let before = kept.board.clone();
            let mut changed = false;
            // A card run by an agent: the board takes it as its worker's, then its terminal starts.
            let prepared = action.and_then(|action| {
                super::board_run::prepared(action, &kept.key, crate::browser::agent_started_on)
            });
            let done = prepared
                .and_then(|(action, run)| {
                    started = run;
                    kept.board.person(&action)
                })
                .and_then(|did| {
                    changed = kept.board != before;
                    if changed {
                        self.save_board(project, &kept.board)?;
                    }
                    Ok(did)
                });
            if done.is_err() {
                // Refused, or not saved: the board stays as it was kept, and nothing starts.
                kept.board = before;
                started = None;
            }
            let refused = match done {
                // What changed nothing (a card dropped where it was) is news to no agent.
                Ok(did) => {
                    if changed {
                        kept.heard.push(did);
                    }
                    None
                }
                Err(why) => Some(board::refused(&why)),
            };
            let frame = kept.frame();
            kept.panel.clone().map(|panel| (panel, frame, refused))
        };
        if let (Some(run), Some(desk)) = (started, self.me.upgrade()) {
            let asked = Asked::Worker {
                command: run.command,
                title: run.title,
                who: run.who,
            };
            desk.person_asked(&crate::projects::key(project), asked);
        }
        if let Some((panel, frame, refused)) = paint {
            panel.paint(&frame);
            if let Some(refused) = refused {
                panel.paint(&refused);
                return;
            }
        }
        self.nudge(project);
    }

    /// project `project`'s board has news: told once the person is quiet.
    fn nudge(&self, project: ProjectId) {
        let mut sender = crate::lock(&self.board_news);
        if sender.is_none() {
            *sender = start_teller(Weak::clone(&self.me));
        }
        let Some(send) = sender.as_ref() else {
            // No thread to wait in: told now.
            drop(sender);
            self.tell_board(project);
            return;
        };
        match send.try_send(project) {
            // A full queue is told soon anyway; the project's news waits in its board.
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                *sender = None;
                drop(sender);
                self.tell_board(project);
            }
        }
    }

    /// What the person did on project `project`'s board since the agent was last told, as one note.
    pub(super) fn tell_board(&self, project: ProjectId) {
        let (key, heard) = {
            let mut boards = crate::lock(&self.boards);
            let Some(kept) = boards.get_mut(&project) else {
                return;
            };
            (kept.key.clone(), std::mem::take(&mut kept.heard))
        };
        // What is said of a card a worker runs is that worker's; the rest is the project's agent's.
        let mut parts: BTreeMap<Option<String>, Vec<Did>> = BTreeMap::new();
        for did in heard {
            let to = (did.who.as_deref()).and_then(|who| self.worker_owner(project, who));
            parts.entry(to).or_default().push(did);
        }
        for (to, heard) in parts {
            if let Some((words, urgent)) = note(&key, heard) {
                self.hear_for(&key, to.as_deref(), &words, urgent == Urgency::Now);
            }
        }
    }
}

/// What the person did, as one note: what wants acting on first, then the quiet news.
fn note(key: &str, mut heard: Vec<Did>) -> Option<(String, Urgency)> {
    heard.sort_by_key(|did| match did.urgent {
        Urgency::Now => 0,
        Urgency::Later => 1,
    });
    let first = heard.first()?;
    let urgent = first.urgent;
    let mut words = format!("The person on the board {key} {}", first.words);
    let rest: Vec<&str> = heard.iter().skip(1).map(|did| did.words.as_str()).collect();
    if !rest.is_empty() {
        words.push_str("\nThey also ");
        words.push_str(&rest.join("; "));
    }
    words.push_str("\n(`look` at the board reads it; a card sent back waits at the top of Todo.)");
    Some((words, urgent))
}

/// The thread that tells the projects' agents their boards' news once the person is quiet; none when the
/// system gives no thread.
fn start_teller(desk: Weak<Desk>) -> Option<SyncSender<ProjectId>> {
    let (send, news) = std::sync::mpsc::sync_channel(WAITING);
    let started = std::thread::Builder::new()
        .name("desk-board-told".into())
        .spawn(move || teller(&desk, &news));
    match started {
        Ok(_) => Some(send),
        Err(why) => {
            log::warn!("the board's news has no thread, and is told at once: {why}");
            None
        }
    }
}

fn teller(desk: &Weak<Desk>, news: &Receiver<ProjectId>) {
    while let Ok(first) = news.recv() {
        let mut projects = BTreeSet::from([first]);
        loop {
            match news.recv_timeout(QUIET) {
                Ok(project) => {
                    projects.insert(project);
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        let Some(desk) = desk.upgrade() else { return };
        for project in &projects {
            desk.tell_board(*project);
        }
    }
}

#[cfg(test)]
#[path = "board_told_tests.rs"]
mod tests;
