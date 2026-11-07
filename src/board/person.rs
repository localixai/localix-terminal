//! What the person does on the board from the window: what a worker may, and what only they may:
//! accept a card into Done, send one back with what is wrong, move any card anywhere (past the
//! limit too), and set the limit. Each action comes to words for the agent, and whether they
//! should hear them now or with their next call.

use serde_json::Value;

use super::text::quote;
use super::{Board, Column, MAX_NOTE, Spot, Step, Worker, now, one_line, said, step, words};

/// What a person's action came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Did {
    /// What they did, as the agent reads it: `returned c4 "Cart" (tests) to Todo: "…"`.
    pub words: String,
    /// Worth telling now (a returned card, a comment), or only with the agent's next call.
    pub urgent: Urgency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Now,
    Later,
}

const PERSON: &str = "person";

fn field<'a>(action: &'a Value, name: &str) -> Option<&'a str> {
    action
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn needed<'a>(action: &'a Value, name: &str) -> Result<&'a str, String> {
    field(action, name).ok_or_else(|| format!("`{name}` is needed"))
}

impl Board {
    /// The person's `action`, as the board's page sends it.
    pub fn person(&mut self, action: &Value) -> Result<Did, String> {
        let op = needed(action, "op")?;
        match op {
            "add" => {
                // What the person writes down lands in Backlog, and waits there: only a card they
                // move to Todo (or add there by name) asks for work.
                let column = match field(action, "to") {
                    Some(name) => Column::parse(name).ok_or("`to` is a column")?,
                    None => Column::Backlog,
                };
                let id = self.add_to(
                    column,
                    needed(action, "title")?,
                    field(action, "note").unwrap_or(""),
                    None,
                    PERSON,
                )?;
                Ok(match column {
                    Column::Backlog | Column::Discarded => self.did(
                        &id,
                        "added",
                        &format!(" to {}", column.title()),
                        Urgency::Later,
                    ),
                    Column::Todo | Column::Doing | Column::Review | Column::Done => {
                        self.did(&id, "added", "", Urgency::Now)
                    }
                })
            }
            "move" => self.person_move(
                needed(action, "card")?,
                Column::parse(needed(action, "to")?).ok_or("`to` is a column")?,
                field(action, "before"),
            ),
            "accept" => {
                let id = needed(action, "card")?;
                self.person_move(id, Column::Done, None)
            }
            "return" => self.send_back(needed(action, "card")?, needed(action, "text")?),
            "comment" => {
                let (id, text) = (needed(action, "card")?, needed(action, "text")?);
                let text = words(text, "text", MAX_NOTE)?;
                let at = self.at(id)?;
                said(&mut self.cards[at], PERSON, &text);
                Ok(self.did(id, "commented on", &format!(": \"{text}\""), Urgency::Now))
            }
            "edit" => self.person_edit(needed(action, "card")?, action),
            "unblock" => {
                let id = needed(action, "card")?;
                self.block(id, Worker(None), None)?;
                Ok(self.did(id, "unblocked", "", Urgency::Now))
            }
            "remove" => {
                let at = self.at(needed(action, "card")?)?;
                let card = self.cards.remove(at);
                let urgent = match card.column {
                    Column::Doing | Column::Review => Urgency::Now,
                    Column::Backlog | Column::Todo | Column::Done | Column::Discarded => {
                        Urgency::Later
                    }
                };
                let words = format!(
                    "removed {} {}{}",
                    card.id,
                    quote(&card.title),
                    owner(card.who.as_deref())
                );
                Ok(Did { words, urgent })
            }
            "limit" => {
                let limit = (action.get("limit").and_then(Value::as_u64))
                    .and_then(|limit| u32::try_from(limit).ok())
                    .filter(|limit| (1..=9).contains(limit))
                    .ok_or("`limit` is 1 to 9")?;
                self.limit = limit;
                let words = format!("set the limit to {limit} card(s) in Doing per worker");
                Ok(Did {
                    words,
                    urgent: Urgency::Later,
                })
            }
            other => Err(format!("no board action `{other}`")),
        }
    }

    /// Card `id` to `to`, before card `before` or at the bottom.
    fn person_move(&mut self, id: &str, to: Column, before: Option<&str>) -> Result<Did, String> {
        let at = self.at(id)?;
        let from = self.cards[at].column;
        let mut card = self.cards.remove(at);
        if from != to {
            card.column = to;
            step(
                &mut card,
                Step {
                    at: now(),
                    by: PERSON.into(),
                    to: Some(to),
                    text: None,
                },
            );
            if to != Column::Doing {
                card.blocked = None;
            }
        }
        let id = card.id.clone();
        self.put(card, before.map_or(Spot::Bottom, Spot::Before));
        Ok(match (from == to, to) {
            (true, _) => self.did(
                &id,
                "reordered",
                &format!(" in {}", to.title()),
                Urgency::Later,
            ),
            (false, Column::Done) => self.did(&id, "accepted", "", Urgency::Later),
            // Put away as an idea: nothing to do on it.
            (false, Column::Backlog) => self.did(
                &id,
                "moved",
                &format!(" from {} to Backlog", from.title()),
                Urgency::Later,
            ),
            (false, _) => {
                let how = format!(" from {} to {}", from.title(), to.title());
                self.did(&id, "moved", &how, Urgency::Now)
            }
        })
    }

    /// Card `id` back to the top of Todo with what is wrong; its worker stays named, as a hint.
    fn send_back(&mut self, id: &str, text: &str) -> Result<Did, String> {
        let text = words(text, "text", MAX_NOTE)?;
        let at = self.at(id)?;
        let mut card = self.cards.remove(at);
        card.column = Column::Todo;
        card.blocked = None;
        card.result = None;
        step(
            &mut card,
            Step {
                at: now(),
                by: PERSON.into(),
                to: Some(Column::Todo),
                text: Some(text.clone()),
            },
        );
        let id = card.id.clone();
        self.put(card, Spot::Top);
        Ok(self.did(
            &id,
            "returned",
            &format!(" to Todo: \"{text}\""),
            Urgency::Now,
        ))
    }

    /// A new title or note for card `id`, wherever it is.
    fn person_edit(&mut self, id: &str, action: &Value) -> Result<Did, String> {
        let at = self.at(id)?;
        let note = action.get("note").and_then(Value::as_str);
        self.rewrite(at, field(action, "title"), note)?;
        let urgent = match self.cards[at].column {
            Column::Doing | Column::Review => Urgency::Now,
            Column::Backlog | Column::Todo | Column::Done | Column::Discarded => Urgency::Later,
        };
        Ok(self.did(id, "edited", "", urgent))
    }

    /// `verb` on card `id`, in words: `returned c4 "Cart" (tests)` and the rest.
    fn did(&self, id: &str, verb: &str, rest: &str, urgent: Urgency) -> Did {
        let words = match self.card(id) {
            Ok(card) => format!(
                "{verb} {} {}{}{rest}",
                card.id,
                quote(&card.title),
                owner(card.who.as_deref())
            ),
            Err(_) => format!("{verb} {id}{rest}"),
        };
        Did { words, urgent }
    }
}

/// ` (tests)` for a worker's card, nothing for the main agent's.
fn owner(who: Option<&str>) -> String {
    who.map_or_else(String::new, |who| format!(" ({})", one_line(who)))
}

#[cfg(test)]
#[path = "person_tests.rs"]
mod tests;
