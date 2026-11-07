//! The board's data and vocabulary, shared by workers, the person and storage.

use serde::{Deserialize, Serialize};

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

    pub(super) fn may_act_on(self, card: &Card) -> bool {
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
