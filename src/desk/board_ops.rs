//! Worker operations on a board: arguments in, domain rules applied, report out.

use serde_json::Value;

use super::args::text;
use crate::board::{self, Board, Worker};

const OPS: &str =
    "look, show, card, add, take, review, block, unblock, release, comment, edit, remove";

/// What `op` does to `board` for `worker`, in words; an empty report for a look.
pub(super) fn worker_op(
    board: &mut Board,
    op: &str,
    args: &Value,
    worker: Worker,
) -> Result<String, String> {
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
