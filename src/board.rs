//! A project's board of cards: what an agent (and its subagents) plan, take, do and hand in, and
//! the person accepts. Four columns, left to right: Todo, Doing, Review, Done. A worker holds at
//! most `limit` cards in Doing (a blocked one waits for the person and does not count); a card
//! reaches Done from the person alone. A worker acts on its own cards, the main agent on any.
//!
//! Here, the board and what a worker may do to it; what the person does is in `person`, the
//! board as a worker reads it in `text`, where it is kept in `store`.

mod page;
mod person;
pub mod store;
mod text;

use serde::{Deserialize, Serialize};

pub use page::{frame, page, refused};
pub use person::{Did, Urgency};
pub use store::place;
pub use text::age;

/// The most cards a board holds, and the longest a title, a note and a comment may be.
const MAX_CARDS: usize = 500;
const MAX_TITLE: usize = 200;
const MAX_NOTE: usize = 8000;
/// The most steps a card's history keeps: its first (how it came), and the latest after it.
const MAX_STEPS: usize = 200;

/// Where a card is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    /// Ideas: not to be done until the person moves one to Todo.
    Backlog,
    Todo,
    Doing,
    Review,
    Done,
    /// Given up: dropped by the person, before or after it was started, and kept to be seen.
    Discarded,
}

impl Column {
    pub const ALL: [Self; 6] = [
        Self::Backlog,
        Self::Todo,
        Self::Doing,
        Self::Review,
        Self::Done,
        Self::Discarded,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Review => "review",
            Self::Done => "done",
            Self::Discarded => "discarded",
        }
    }

    /// Its name as the board shows it.
    pub fn title(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Todo => "Todo",
            Self::Doing => "Doing",
            Self::Review => "Review",
            Self::Done => "Done",
            Self::Discarded => "Discarded",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|column| column.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// One thing that happened to a card: a move, or words about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// Seconds since the epoch.
    pub at: u64,
    /// `person`, `agent` (the main agent) or a worker's name.
    pub by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Column>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    /// `c1`, `c2`…, never used again on this board.
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub column: Column,
    /// Who works on it: a worker's name; none is the main agent, or nobody yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    /// What it waits for, while it is blocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
    /// What its worker said was done, when they handed it in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// A tab of the window it is about (`w1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(default)]
    pub history: Vec<Step>,
}

impl Card {
    /// When it came to the column it is in.
    pub fn since(&self) -> Option<u64> {
        let moved = self.history.iter().rev().find(|step| step.to.is_some());
        moved.map(|step| step.at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The number the next card takes.
    pub next: u32,
    /// The most cards one worker has in Doing.
    pub limit: u32,
    pub cards: Vec<Card>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            next: 1,
            limit: 1,
            cards: vec![],
        }
    }
}

/// Who acts on the board through a call: the main agent, or a worker by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Worker<'a>(pub Option<&'a str>);

impl Worker<'_> {
    /// How the board writes it: the worker's name, or `agent`.
    pub fn name(self) -> String {
        self.0.unwrap_or("agent").to_string()
    }

    fn may_act_on(self, card: &Card) -> bool {
        self.0.is_none() || card.who.as_deref() == self.0
    }
}

/// A worker's name as it is given: a short word, not one the board keeps for itself.
pub fn worker_name(name: Option<&str>) -> Result<Option<&str>, String> {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    let fits = name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if !fits || ["person", "agent"].contains(&name) {
        return Err(format!(
            "`who` is a worker's short name (letters, digits, - _ . :), not `{name}`"
        ));
    }
    Ok(Some(name))
}

/// Now, in seconds since the epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
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
        let id = id.trim();
        (self.cards.iter())
            .find(|card| card.id.eq_ignore_ascii_case(id))
            .ok_or_else(|| format!("there is no card {id} on the board"))
    }

    fn at(&self, id: &str) -> Result<usize, String> {
        let found = self.card(id)?.id.clone();
        (self.cards.iter())
            .position(|card| card.id == found)
            .ok_or_else(|| format!("there is no card {id} on the board"))
    }

    /// A new card at the bottom of Todo; its id.
    pub fn add(
        &mut self,
        title: &str,
        note: &str,
        tab: Option<&str>,
        by: &str,
    ) -> Result<String, String> {
        self.add_to(Column::Todo, title, note, tab, by)
    }

    /// A new card at the bottom of `column`, Todo or Backlog (an idea); its id.
    pub fn add_to(
        &mut self,
        column: Column,
        title: &str,
        note: &str,
        tab: Option<&str>,
        by: &str,
    ) -> Result<String, String> {
        if !matches!(column, Column::Todo | Column::Backlog) {
            return Err(format!(
                "a card is added to Todo or to Backlog, not to {}",
                column.title()
            ));
        }
        if self.cards.len() >= MAX_CARDS {
            return Err(format!(
                "the board holds {MAX_CARDS} cards: remove some first"
            ));
        }
        let title = words(title, "title", MAX_TITLE)?;
        let note = if note.trim().is_empty() {
            String::new()
        } else {
            words(note, "note", MAX_NOTE)?
        };
        let id = format!("c{}", self.next);
        self.next = self.next.saturating_add(1);
        self.cards.push(Card {
            id: id.clone(),
            title,
            note,
            column,
            who: None,
            blocked: None,
            result: None,
            tab: tab.map(String::from),
            history: vec![Step {
                at: now(),
                by: by.to_string(),
                to: Some(column),
                text: None,
            }],
        });
        Ok(id)
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
        let title = title.map(|t| words(t, "title", MAX_TITLE)).transpose()?;
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

    /// Where card `id` is, when `worker` may act on it.
    /// Card `id`, which `worker` may hand in now: its own, in Doing. Nothing changes.
    pub fn may_hand_in(&self, id: &str, worker: Worker) -> Result<&Card, String> {
        Ok(&self.cards[self.handed_in(id, worker)?])
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

/// `step` onto `card`'s history, the oldest after its first let go past [`MAX_STEPS`].
fn step(card: &mut Card, step: Step) {
    card.history.push(step);
    let over = card.history.len().saturating_sub(MAX_STEPS);
    if over > 0 {
        card.history.drain(1..=over);
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
