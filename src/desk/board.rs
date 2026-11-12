//! A project's board as a tab of the window (`b1`): the ops a worker plans, takes, hands in and
//! reads cards by, and the board's panel, painted whole whenever the board changes. The board is
//! kept in the desk's database, written with the notice that it changed (`save_board`); a
//! project's is written out to its file by the `board-file` projector. What the person does on it
//! is in `board_told`.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use serde_json::Value;

use crate::board::{self, Board, Worker};
use crate::browser::{Pane, Panel};
use crate::feed::{Entity, Op};
use crate::projects::ProjectId;

use super::args::text;
use super::board_ops::worker_op;
use super::{Call, Desk, Failure, Output};

/// The person's actions on a board that wait to be worked; more than a person makes at once.
const ACTIONS_WAITING: usize = 64;

/// What a board's worker thread is handed, in the order it came.
enum Work {
    /// The person did this in the page.
    Posted(String),
    /// The page loaded, late or again: the board is painted whole.
    Repaint,
}

/// A project's board as the desk holds it.
pub(super) struct Kept {
    /// Its tab: `b1`.
    pub(super) key: String,
    pub(super) board: Board,
    /// The project's name, as the board is titled.
    pub(super) project: String,
    pub(super) panel: Option<Arc<Panel>>,
    /// What the person did on it that the agent was not told yet.
    pub(super) heard: Vec<board::Did>,
}

impl Kept {
    /// The frame that shows the board as it is now, with the agents installed to run a card by.
    pub(super) fn frame(&self) -> String {
        let agents = serde_json::from_str(&crate::browser::installed_agents()).unwrap_or_default();
        board::frame(&self.board, &self.key, &self.project, board::now(), &agents)
    }
}

impl Desk {
    /// A `board` call: `op` on the project's board, and the board as the worker reads it after.
    pub(super) fn board(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        // A named agent is that worker on the board, with no `who` of its own to pass.
        let worker = Worker(board::worker_name(text(args, "who").or(call.agent))?);
        let op = text(args, "op").unwrap_or("look");
        let (report, state, paint, key) = {
            let mut boards = crate::lock(&self.boards);
            let kept = match boards.entry(call.project.id) {
                std::collections::hash_map::Entry::Occupied(kept) => kept.into_mut(),
                std::collections::hash_map::Entry::Vacant(none) => {
                    none.insert(self.new_board(call)?)
                }
            };
            // What a look, a card read whole or a show does not change is not copied to be
            // compared; an op that changes it is restored to what it was when it fails midway
            // (`add` of several cards, the third without a title).
            let before = (!matches!(op, "look" | "show" | "card")).then(|| kept.board.clone());
            let report = match worker_op(&mut kept.board, op, args, worker) {
                Ok(report) => report,
                Err(why) => {
                    if let Some(before) = before {
                        kept.board = before;
                    }
                    return Err(why.into());
                }
            };
            let changed = before.as_ref().is_some_and(|before| kept.board != *before);
            if changed && let Err(why) = self.save_board(call.project.id, &kept.board) {
                if let Some(before) = before {
                    kept.board = before;
                }
                return Err(why.into());
            }
            let state = kept.board.text(&kept.key, worker, board::now());
            let paint = (changed || op == "show")
                .then(|| kept.panel.clone().map(|panel| (panel, kept.frame())))
                .flatten();
            (report, state, paint, kept.key.clone())
        };
        if let Some((panel, frame)) = paint {
            panel.paint(&frame);
        }
        if op == "show" && self.browser.is_some() && self.reveal(call, &key).is_err() {
            // The window went with the board's tab (the person closed its last one): the board
            // gets a tab again, painted as it is.
            self.reopen_board(call);
            self.reveal(call, &key)?;
        }
        Ok(Output {
            report: if report.is_empty() {
                key
            } else {
                format!("{key}: {report}")
            },
            state: Some(state),
            shot: None,
        })
    }

    /// Project `project`'s board saved as `board`, with the notice that it changed: both or
    /// neither.
    pub(super) fn save_board(&self, project: ProjectId, board: &Board) -> Result<(), String> {
        self.feed.write(|write| {
            board::store::save(write.tx(), project, board)?;
            write.record(&Entity::Board { project }, Op::Upserted)
        })
    }

    /// The project's board, read from its row, in a tab of its own where a window shows. A
    /// project whose board has no row yet takes the one its file kept before the database.
    fn new_board(&self, call: &Call) -> Result<Kept, String> {
        let id = call.project.id;
        let kept = (self.feed.store()).read(|db| board::store::load(db, id))?;
        let board = match kept {
            Some(board) => board,
            None => {
                let before = board::store::written(&call.project.path)?.unwrap_or_default();
                if before != Board::default() {
                    self.save_board(id, &before)?;
                }
                before
            }
        };
        let n = self.next_board.fetch_add(1, Ordering::SeqCst);
        let key = format!("b{n}");
        self.own(
            &key,
            call,
            crate::ledger::Kind::Board,
            super::ledger::Facts {
                title: "Board",
                ..super::ledger::Facts::default()
            },
        );
        let project = call.project.name.clone();
        let aside = self.quiet(call);
        let panel = self.board_panel(n, (&key, aside), &project, id);
        let kept = Kept {
            key,
            board,
            project,
            panel,
            heard: vec![],
        };
        if let Some(panel) = &kept.panel {
            panel.paint(&kept.frame());
        }
        Ok(kept)
    }

    /// The call's project's board in a new tab, its old one gone with the window: a pane of the
    /// project's ledger and layout again, as it was when the board first opened.
    fn reopen_board(&self, call: &Call) {
        let project = call.project.id;
        let mut boards = crate::lock(&self.boards);
        let Some(kept) = boards.get_mut(&project) else {
            return;
        };
        let Some(n) = kept.key.strip_prefix('b').and_then(|n| n.parse().ok()) else {
            return;
        };
        let facts = super::ledger::Facts {
            title: "Board",
            ..super::ledger::Facts::default()
        };
        self.own(&kept.key, call, crate::ledger::Kind::Board, facts);
        // The old panel let go first: it shares the new one's id.
        drop(kept.panel.take());
        let aside = self.quiet(call);
        kept.panel = self.board_panel(n, (&kept.key, aside), &kept.project, project);
        if let Some(panel) = &kept.panel {
            panel.paint(&kept.frame());
        }
    }

    /// The board's panel: what the person does in it comes back as their actions, and a page that
    /// loads late is painted whole.
    ///
    /// `aside`, its tab opens beside the tab in sight and does not take the window.
    fn board_panel(
        &self,
        n: u32,
        (key, aside): (&str, bool),
        name: &str,
        project: ProjectId,
    ) -> Option<Arc<Panel>> {
        let browser = self.browser.as_ref()?;
        let id = crate::browser::BOARD.checked_add(n)?;
        let title = format!("desk · board · {name}");
        browser.group(key, &crate::projects::key(project));
        if aside {
            browser.aside(key);
        }
        let panel = Arc::new(browser.panel(id, &title, board::page(&title)));
        let me = Weak::clone(&self.me);
        // The window's loop calls these: the board is worked, saved and painted on a thread of its
        // own, the person's actions and the page's loads in the order they came; it ends with the
        // panel.
        let (to_worker, actions) = std::sync::mpsc::sync_channel::<Work>(ACTIONS_WAITING);
        let tab = key.to_string();
        let worker = std::thread::Builder::new()
            .name("desk-person-board".into())
            .spawn(move || {
                for work in actions {
                    let Some(desk) = me.upgrade() else { return };
                    match work {
                        Work::Posted(posted) => desk.person_on_board(project, &posted),
                        Work::Repaint => desk.repaint_board(&tab),
                    }
                }
            });
        if let Err(why) = worker {
            log::warn!("the board takes no action from the person: {why}");
        }
        let loaded = to_worker.clone();
        panel.keyboard(Arc::new(move |posted: &str| {
            if to_worker
                .try_send(Work::Posted(posted.to_string()))
                .is_err()
            {
                log::warn!("the person's action on the board was dropped: too many wait");
            }
        }));
        panel.sizing(Arc::new(move |pane, _, _| {
            if pane == Pane::Loaded && loaded.try_send(Work::Repaint).is_err() {
                log::warn!("the board was not painted as its page loaded: too many wait");
            }
        }));
        Some(panel)
    }

    /// The board shown in tab `key` painted whole again.
    pub(super) fn repaint_board(&self, key: &str) {
        let paint = {
            let boards = crate::lock(&self.boards);
            let kept = boards.values().find(|kept| kept.key == key);
            kept.and_then(|kept| Some((kept.panel.clone()?, kept.frame())))
        };
        if let Some((panel, frame)) = paint {
            panel.paint(&frame);
        }
    }

    /// Whether tab `key` is project `project`'s board.
    pub(super) fn board_of(&self, project: ProjectId, key: &str) -> bool {
        (crate::lock(&self.boards).get(&project)).is_some_and(|kept| kept.key == key)
    }

    /// The board in tab `key`, as the main agent reads it.
    pub(super) fn board_text(&self, key: &str) -> Result<String, String> {
        let boards = crate::lock(&self.boards);
        let kept = (boards.values().find(|kept| kept.key == key))
            .ok_or_else(|| format!("there is no board {key}: it was closed"))?;
        Ok(kept.board.text(&kept.key, Worker(None), board::now()))
    }

    /// Board `key` in a few words, for `list`.
    pub(super) fn board_summary(&self, key: &str) -> String {
        let boards = crate::lock(&self.boards);
        let kept = boards.values().find(|kept| kept.key == key);
        kept.map(|kept| kept.board.summary()).unwrap_or_default()
    }

    /// Board `key`'s tab closed; the board itself stays in its file.
    pub(super) fn close_board(&self, key: &str) -> Result<String, String> {
        let mut boards = crate::lock(&self.boards);
        let project = (boards.iter())
            .find(|(_, kept)| kept.key == key)
            .map(|(project, _)| *project)
            .ok_or_else(|| format!("there is no board {key}"))?;
        boards.remove(&project);
        Ok(format!(
            "Closed {key}; the board is kept, and opens again as it was."
        ))
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
