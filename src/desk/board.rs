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
use super::{Call, Desk, Failure, Output};

/// The person's actions on a board that wait to be worked; more than a person makes at once.
const ACTIONS_WAITING: usize = 64;

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
    /// The frame that shows the board as it is now.
    pub(super) fn frame(&self) -> String {
        board::frame(&self.board, &self.key, &self.project, board::now())
    }
}

const OPS: &str =
    "look, show, card, add, take, review, block, unblock, release, comment, edit, remove";

impl Desk {
    /// A `board` call: `op` on the project's board, and the board as the worker reads it after.
    pub(super) fn board(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let worker = Worker(board::worker_name(text(args, "who"))?);
        let op = text(args, "op").unwrap_or("look");
        let (report, state, paint, key) = {
            let mut boards = crate::lock(&self.boards);
            let kept = match boards.entry(call.project.id) {
                std::collections::hash_map::Entry::Occupied(kept) => kept.into_mut(),
                std::collections::hash_map::Entry::Vacant(none) => {
                    none.insert(self.new_board(call)?)
                }
            };
            let before = kept.board.clone();
            let report = worker_op(&mut kept.board, op, args, worker)?;
            let changed = kept.board != before;
            if changed && let Err(why) = self.save_board(call.project.id, &kept.board) {
                kept.board = before;
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
        if op == "show"
            && let Some(browser) = &self.browser
            && browser.act(&key, false).is_err()
        {
            // The window went with the board's tab (the person closed its last one): the board
            // gets a tab again, painted as it is.
            self.reopen_board(call.project.id);
            browser.act(&key, false)?;
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
        let panel = self.board_panel(n, &key, &project, id);
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

    /// Project `project`'s board in a new tab, its old one gone with the window.
    fn reopen_board(&self, project: ProjectId) {
        let mut boards = crate::lock(&self.boards);
        let Some(kept) = boards.get_mut(&project) else {
            return;
        };
        let Some(n) = kept.key.strip_prefix('b').and_then(|n| n.parse().ok()) else {
            return;
        };
        // The old panel let go first: it shares the new one's id.
        drop(kept.panel.take());
        kept.panel = self.board_panel(n, &kept.key, &kept.project, project);
        if let Some(panel) = &kept.panel {
            panel.paint(&kept.frame());
        }
    }

    /// The board's panel: what the person does in it comes back as their actions, and a page that
    /// loads late is painted whole.
    fn board_panel(&self, n: u32, key: &str, name: &str, project: ProjectId) -> Option<Arc<Panel>> {
        let browser = self.browser.as_ref()?;
        let id = crate::browser::BOARD.checked_add(n)?;
        let title = format!("desk · board · {name}");
        browser.group(key, &crate::projects::key(project));
        let panel = Arc::new(browser.panel(id, &title, board::page(&title)));
        let me = Weak::clone(&self.me);
        // The window's loop calls this: the board is worked (and saved) on a thread of its own,
        // the person's actions in the order they came; it ends with the panel.
        let (to_worker, actions) = std::sync::mpsc::sync_channel::<String>(ACTIONS_WAITING);
        let worker = std::thread::Builder::new()
            .name("desk-person-board".into())
            .spawn(move || {
                for posted in actions {
                    match me.upgrade() {
                        Some(desk) => desk.person_on_board(project, &posted),
                        None => return,
                    }
                }
            });
        if let Err(why) = worker {
            log::warn!("the board takes no action from the person: {why}");
        }
        panel.keyboard(Arc::new(move |posted: &str| {
            if to_worker.try_send(posted.to_string()).is_err() {
                log::warn!("the person's action on the board was dropped: too many wait");
            }
        }));
        let (me, key) = (Weak::clone(&self.me), key.to_string());
        panel.sizing(Arc::new(move |pane, _, _| {
            if pane == Pane::Loaded
                && let Some(desk) = me.upgrade()
            {
                desk.repaint_board(&key);
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

/// What `op` does to `board` for `worker`, in words; an empty report for a look.
fn worker_op(board: &mut Board, op: &str, args: &Value, worker: Worker) -> Result<String, String> {
    let card = || {
        text(args, "card").ok_or_else(|| format!("`card` is needed for `{op}`: an id such as c3"))
    };
    let note = text(args, "note");
    match op {
        "look" | "show" => Ok(String::new()),
        "card" => board.card_text(card()?, board::now()),
        "add" => add(board, args, worker),
        "take" => {
            // A card already the worker's in Doing is said so: the person may have moved it there.
            let had = text(args, "card")
                .and_then(|id| board.card(id).ok())
                .is_some_and(|card| {
                    card.column == board::Column::Doing && card.who.as_deref() == worker.0
                });
            let id = board.take(text(args, "card"), worker)?;
            let whole = board.card_text(&id, board::now())?;
            let said = if had {
                let by = (board.card(&id).ok())
                    .and_then(|card| card.history.last())
                    .map(|step| step.by.as_str());
                match by {
                    Some("person") => {
                        format!("{id} is yours already: the person moved it to Doing")
                    }
                    _ => format!("{id} is yours already, in Doing"),
                }
            } else {
                format!("took {id}")
            };
            Ok(format!(
                "{said}: it is {}'s until it is handed in with `review`.\n\n{whole}",
                worker.name()
            ))
        }
        "review" => {
            let id = card()?;
            board.review(id, worker, note.unwrap_or(""))?;
            Ok(format!(
                "handed {id} in: it waits in Review for the person to accept or send back."
            ))
        }
        "block" => {
            let id = card()?;
            let reason = text(args, "reason")
                .or(note)
                .ok_or("`reason` says what the card waits for")?;
            board.block(id, worker, Some(reason))?;
            Ok(format!(
                "{id} is blocked; the person sees what it waits for."
            ))
        }
        "unblock" => board
            .block(card()?, worker, None)
            .map(|()| "unblocked.".into()),
        "release" => board
            .release(card()?, worker, note)
            .map(|()| "released: back in Todo.".into()),
        "comment" => {
            let said = note.ok_or("`note` is what to say about the card")?;
            board
                .comment(card()?, worker, said)
                .map(|()| "noted.".into())
        }
        "edit" => {
            let id = card()?;
            board.edit(
                id,
                worker,
                text(args, "title"),
                args.get("note").and_then(Value::as_str),
            )?;
            Ok(format!("edited {id}."))
        }
        "remove" => board
            .remove(card()?, worker)
            .map(|card| format!("removed {}.", card.id)),
        other => Err(format!("no board op `{other}`: {OPS}")),
    }
}

/// One card (`title`, `note`, `tab`) or several (`cards`, each with its `title` and `note`).
fn add(board: &mut Board, args: &Value, worker: Worker) -> Result<String, String> {
    let (tab, by) = (text(args, "tab"), worker.name());
    // Todo, or Backlog for an idea the person may take up later.
    let column = match text(args, "to") {
        Some(name) => board::Column::parse(name).ok_or("`to` is todo or backlog")?,
        None => board::Column::Todo,
    };
    let mut added = vec![];
    if let Some(cards) = args.get("cards").and_then(Value::as_array) {
        for card in cards {
            let title = text(card, "title").ok_or("each of `cards` has a `title`")?;
            added.push(board.add_to(
                column,
                title,
                text(card, "note").unwrap_or(""),
                text(card, "tab").or(tab),
                &by,
            )?);
        }
    }
    if let Some(title) = text(args, "title") {
        added.push(board.add_to(column, title, text(args, "note").unwrap_or(""), tab, &by)?);
    }
    if added.is_empty() {
        return Err("`title` (one card) or `cards` (several) is needed for `add`".into());
    }
    Ok(format!(
        "added {} to the bottom of {}.",
        added.join(", "),
        column.title()
    ))
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
