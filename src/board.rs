//! A project's board of cards: what an agent (and its subagents) plan, take, do and hand in, and
//! the person accepts. Six columns, left to right: Backlog, Todo, Doing, Review, Done, Discarded.
//! A worker holds at most `limit` cards in Doing (a blocked one waits for the person and does not
//! count); a card reaches Done from the person alone. A worker acts on its own cards, the main
//! agent on any.
//!
//! Here, the board and what a worker may do to it; what the person does is in `person`, the
//! board as a worker reads it in `text`, where it is kept in `store`.

mod cards;
mod model;
mod page;
mod person;
pub mod store;
mod text;

pub use model::{Board, Card, Column, Step, Worker, worker_name};

pub use page::{frame, page, refused};
pub use person::{Did, Urgency};
pub use store::place;

/// The most cards a board holds, and the longest a title, a note and a comment may be.
const MAX_CARDS: usize = 500;
const MAX_TITLE: usize = 200;
const MAX_NOTE: usize = 8000;
/// The most steps a card's history keeps: its first (how it came), and the latest after it.
const MAX_STEPS: usize = 200;

/// Now, in seconds since the epoch: the desk's one clock ([`crate::now`]).
pub use crate::now;

/// `text` on one line: its lines and runs of white space (and any control character) become one
/// space. A title is one line, so it cannot pass for lines of its own in what an agent is told.
fn one_line(text: &str) -> String {
    (text.split(|c: char| c.is_whitespace() || c.is_control()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A card's title: `title` on one line, refused when it says nothing or is too long.
fn heading(title: &str) -> Result<String, String> {
    words(&one_line(title), "title", MAX_TITLE)
}

/// `text` trimmed, refused when it says nothing or is longer than `most` bytes.
fn words(text: &str, field: &str, most: usize) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("`{field}` says nothing"));
    }
    if text.len() > most {
        return Err(format!("`{field}` is at most {most} bytes"));
    }
    Ok(text.to_string())
}

impl Board {
    pub fn card(&self, id: &str) -> Result<&Card, String> {
        self.at(id).map(|at| &self.cards[at])
    }

    /// Where card `id` is: the one lookup of a card by its id.
    fn at(&self, id: &str) -> Result<usize, String> {
        let id = id.trim();
        (self.cards.iter())
            .position(|card| card.id.eq_ignore_ascii_case(id))
            .ok_or_else(|| format!("there is no card {id} on the board"))
    }

    /// `worker` takes card `id` (the top of Todo when none is named) into Doing.
    pub fn take(&mut self, id: Option<&str>, worker: Worker) -> Result<String, String> {
        let at = match id {
            Some(id) => self.at(id)?,
            None => (self.cards.iter())
                .position(|card| card.column == Column::Todo)
                .ok_or("Todo is empty: nothing to take")?,
        };
        let card = &self.cards[at];
        let mine = card.who.as_deref() == worker.0;
        match card.column {
            Column::Doing if mine => return Ok(card.id.clone()),
            Column::Doing => {
                return Err(format!(
                    "{} is being done by {}",
                    card.id,
                    Worker(card.who.as_deref()).name()
                ));
            }
            Column::Review | Column::Done | Column::Discarded => {
                return Err(format!(
                    "{} is in {}: only a card in Todo is taken",
                    card.id,
                    card.column.name()
                ));
            }
            Column::Backlog => {
                return Err(format!(
                    "{} is an idea in Backlog: it is done once the person moves it to Todo",
                    card.id
                ));
            }
            Column::Todo => {}
        }
        if let Some(held) = self.held_by(worker).filter(|held| held.len() >= self.cap()) {
            return Err(format!(
                "{} already has {} in Doing (at most {} each): hand it in with `review`, mark it \
                 `block`ed, or `release` it first",
                worker.name(),
                held.join(", "),
                self.limit
            ));
        }
        let card = &mut self.cards[at];
        card.who = worker.0.map(String::from);
        card.blocked = None;
        let id = card.id.clone();
        self.moved(at, Column::Doing, &worker.name(), None);
        Ok(id)
    }

    /// The cards `worker` has in Doing that count against its limit.
    fn held_by(&self, worker: Worker) -> Option<Vec<String>> {
        let held: Vec<String> = (self.cards.iter())
            .filter(|card| card.column == Column::Doing && card.blocked.is_none())
            .filter(|card| card.who.as_deref() == worker.0)
            .map(|card| card.id.clone())
            .collect();
        (!held.is_empty()).then_some(held)
    }

    fn cap(&self) -> usize {
        usize::try_from(self.limit.max(1)).unwrap_or(1)
    }

    /// `worker` hands card `id` in for the person to review, saying what was done.
    pub fn review(&mut self, id: &str, worker: Worker, result: &str) -> Result<(), String> {
        let result = words(result, "note", MAX_NOTE).map_err(|why| {
            format!("{why}: it says what was done, for the person who reviews it")
        })?;
        let at = self.handed_in(id, worker)?;
        let card = &mut self.cards[at];
        card.blocked = None;
        card.result = Some(result.clone());
        self.moved(at, Column::Review, &worker.name(), Some(result));
        Ok(())
    }

    /// Card `id` waits for `reason`, or no longer waits.
    pub fn block(&mut self, id: &str, worker: Worker, reason: Option<&str>) -> Result<(), String> {
        let reason = reason.map(|r| words(r, "reason", MAX_TITLE)).transpose()?;
        let at = self.own(id, worker)?;
        let card = &mut self.cards[at];
        if matches!(
            card.column,
            Column::Review | Column::Done | Column::Discarded
        ) {
            return Err(format!("{} is in {}", card.id, card.column.name()));
        }
        card.blocked.clone_from(&reason);
        let text = reason.map_or_else(|| "unblocked".to_string(), |r| format!("blocked: {r}"));
        said(card, &worker.name(), &text);
        Ok(())
    }

    /// `worker` gives card `id` back to Todo, for anyone to take.
    ///
    /// `note`, when given, says where it stands, for whoever takes it next; it is kept in its history.
    pub fn release(&mut self, id: &str, worker: Worker, note: Option<&str>) -> Result<(), String> {
        let note = note
            .filter(|note| !note.trim().is_empty())
            .map(|note| words(note, "note", MAX_NOTE))
            .transpose()?;
        let at = self.own(id, worker)?;
        let card = &mut self.cards[at];
        if card.column != Column::Doing {
            return Err(format!("{} is not in Doing", card.id));
        }
        card.who = None;
        self.moved(at, Column::Todo, &worker.name(), note);
        Ok(())
    }

    /// Words about card `id`, kept in its history.
    pub fn comment(&mut self, id: &str, worker: Worker, text: &str) -> Result<(), String> {
        let text = words(text, "note", MAX_NOTE)?;
        let at = self.own(id, worker)?;
        said(&mut self.cards[at], &worker.name(), &text);
        Ok(())
    }

    /// A new title or note for card `id`, before it is done.
    pub fn edit(
        &mut self,
        id: &str,
        worker: Worker,
        title: Option<&str>,
        note: Option<&str>,
    ) -> Result<(), String> {
        let at = self.own(id, worker)?;
        if matches!(self.cards[at].column, Column::Done | Column::Discarded) {
            let card = &self.cards[at];
            return Err(format!("{} is in {}: closed", card.id, card.column.title()));
        }
        self.rewrite(at, title, note)
    }

    /// The card at `at` with a new title or note.
    fn rewrite(
        &mut self,
        at: usize,
        title: Option<&str>,
        note: Option<&str>,
    ) -> Result<(), String> {
        let title = title.map(heading).transpose()?;
        let note = note.map(|n| n.trim().to_string());
        if note.as_ref().is_some_and(|n| n.len() > MAX_NOTE) {
            return Err(format!("`note` is at most {MAX_NOTE} bytes"));
        }
        let card = &mut self.cards[at];
        if let Some(title) = title {
            card.title = title;
        }
        if let Some(note) = note {
            card.note = note;
        }
        Ok(())
    }

    /// Card `id` off the board: a worker removes only what is still in Todo.
    pub fn remove(&mut self, id: &str, worker: Worker) -> Result<Card, String> {
        let at = self.at(id)?;
        let card = &self.cards[at];
        let waiting = matches!(card.column, Column::Todo | Column::Backlog);
        if !waiting || !worker.may_act_on(card) {
            return Err(format!(
                "{} is in {}: only a card in Todo or Backlog is removed, by whoever may act on it",
                card.id,
                card.column.name()
            ));
        }
        Ok(self.cards.remove(at))
    }

    /// Where card `id` is, which `worker` may hand in: its own, in Doing.
    fn handed_in(&self, id: &str, worker: Worker) -> Result<usize, String> {
        let at = self.own(id, worker)?;
        let card = &self.cards[at];
        if card.column != Column::Doing {
            return Err(format!(
                "{} is in {}: a card is handed in from Doing (`take` it first)",
                card.id,
                card.column.name()
            ));
        }
        Ok(at)
    }

    /// Where card `id` is, when `worker` may act on it.
    fn own(&self, id: &str, worker: Worker) -> Result<usize, String> {
        let at = self.at(id)?;
        let card = &self.cards[at];
        if !worker.may_act_on(card) {
            return Err(format!(
                "{} is {}'s: a worker acts on its own cards",
                card.id,
                Worker(card.who.as_deref()).name()
            ));
        }
        Ok(at)
    }

    /// The card at `at` moved to `to` by `by`, with what was said, to the bottom of its column.
    fn moved(&mut self, at: usize, to: Column, by: &str, text: Option<String>) {
        let mut card = self.cards.remove(at);
        card.column = to;
        step(
            &mut card,
            Step {
                at: now(),
                by: by.to_string(),
                to: Some(to),
                text,
            },
        );
        self.put(card, Spot::Bottom);
    }

    /// `card` into its column: at its top, at its bottom, or just before another of its cards.
    fn put(&mut self, card: Card, spot: Spot) {
        let column = card.column;
        let at = match spot {
            Spot::Before(id) => (self.cards.iter())
                .position(|other| other.id.eq_ignore_ascii_case(id) && other.column == column),
            Spot::Top => self.cards.iter().position(|other| other.column == column),
            Spot::Bottom => None,
        };
        let last = (self.cards.iter())
            .rposition(|other| other.column == column)
            .map(|last| last + 1);
        let at = at.or(last).unwrap_or(self.cards.len());
        self.cards.insert(at, card);
    }
}

/// Where in its column a card goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot<'a> {
    Top,
    Bottom,
    Before(&'a str),
}

fn said(card: &mut Card, by: &str, text: &str) {
    step(
        card,
        Step {
            at: now(),
            by: by.to_string(),
            to: None,
            text: Some(text.to_string()),
        },
    );
}

/// `step` onto `card`'s history, past [`MAX_STEPS`] the oldest words after its first let go: its
/// moves last longer, since the latest one dates the card in its column. Only when it has no
/// words to let go does the oldest move after its first.
fn step(card: &mut Card, step: Step) {
    card.history.push(step);
    let mut over = card.history.len().saturating_sub(MAX_STEPS);
    if over == 0 {
        return;
    }
    let mut at = 0;
    card.history.retain(|step| {
        at += 1;
        let drop = at > 1 && over > 0 && step.to.is_none();
        if drop {
            over -= 1;
        }
        !drop
    });
    if over > 0 {
        card.history.drain(1..=over);
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
