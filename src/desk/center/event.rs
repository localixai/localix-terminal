//! What an event of the Event Center is, and how it is shown: its kind (its section, tone, and
//! whether it wants the person), its words, and its buttons, each with the role the window places
//! it by. The window draws an event as it comes and holds no table of kinds.

use serde::Serialize;

/// What an event is. The order here is the order the sections are shown in: what wants the person
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::desk::center) enum Kind {
    Ask,
    Waiting,
    Review,
    Blocked,
    Failed,
    Finished,
}

/// The colour an event is drawn in: the window's own, named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::desk::center) enum Tone {
    Purple,
    Blue,
    Orange,
    Red,
    Green,
}

/// How an event of a kind is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::desk::center) struct Look {
    /// The heading of its section, and the section's place.
    pub(in crate::desk::center) section: &'static str,
    pub(in crate::desk::center) rank: u8,
    pub(in crate::desk::center) tone: Tone,
    /// It wants the person. What ended well is listed quietly.
    pub(in crate::desk::center) urgent: bool,
}

impl Kind {
    /// Its name, as the journal keeps it: `review`, `finished`.
    pub(in crate::desk::center) fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|name| name.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    pub(in crate::desk::center) fn look(self) -> Look {
        let (section, tone, urgent) = match self {
            Kind::Ask => ("The Agent Asks You", Tone::Purple, true),
            Kind::Waiting => ("Waiting for You", Tone::Purple, true),
            Kind::Review => ("Ready for Review", Tone::Blue, true),
            Kind::Blocked => ("Blocked", Tone::Orange, true),
            Kind::Failed => ("Programs Ended", Tone::Red, true),
            Kind::Finished => ("Programs Finished", Tone::Green, false),
        };
        Look {
            section,
            rank: self as u8,
            tone,
            urgent,
        }
    }
}

/// What an event's button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::desk::center) enum Do {
    /// A card in Review accepted into Done, as the board's Accept.
    Accept {
        project: crate::projects::ProjectId,
        card: String,
    },
    /// A blocked card unblocked, as the board's Unblock.
    Unblock {
        project: crate::projects::ProjectId,
        card: String,
    },
    /// The board of the project grouped as `group`, in front (opened again if it was closed).
    Board { group: String },
    /// A tab in front.
    Show { key: String },
    /// A tab closed, as its x.
    Close { key: String },
    /// What the agent asked on a tab is done: it hears so, and the ask goes.
    Answer { tab: String },
    /// Gone from the center until it changes.
    Dismiss,
}

/// What a button is for, so the window places it: the row's own click, its one button, or its x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::desk::center) enum Role {
    Open,
    Act,
    Dismiss,
}

impl Do {
    pub(in crate::desk::center) fn role(&self) -> Role {
        match self {
            Do::Board { .. } | Do::Show { .. } => Role::Open,
            Do::Accept { .. } | Do::Unblock { .. } | Do::Close { .. } | Do::Answer { .. } => {
                Role::Act
            }
            Do::Dismiss => Role::Dismiss,
        }
    }
}

/// A button of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::desk::center) struct Button {
    pub(in crate::desk::center) label: &'static str,
    pub(in crate::desk::center) role: Role,
    #[serde(skip)]
    pub(in crate::desk::center) does: Do,
}

/// One event as the strip shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::desk::center) struct Event {
    /// The number the strip says back for it.
    pub(in crate::desk::center) n: u64,
    #[serde(skip)]
    pub(in crate::desk::center) id: String,
    #[serde(skip)]
    pub(in crate::desk::center) kind: Kind,
    #[serde(flatten)]
    pub(in crate::desk::center) look: Look,
    /// The project it is of.
    pub(in crate::desk::center) group: String,
    /// The tab it is about, if any.
    pub(in crate::desk::center) tab: String,
    pub(in crate::desk::center) title: String,
    pub(in crate::desk::center) detail: String,
    /// When it happened (seconds since the epoch; none: when the center first saw it).
    pub(in crate::desk::center) at: u64,
    /// In order: the strip says back the place of the one clicked.
    pub(in crate::desk::center) buttons: Vec<Button>,
    /// The person has not seen it yet.
    pub(in crate::desk::center) unseen: bool,
}

impl Event {
    /// An event of `kind` in project `group`, about `tab` (none: ""), known by `id`.
    pub(in crate::desk::center) fn new(id: String, kind: Kind, group: &str, tab: &str) -> Self {
        Event {
            n: 0,
            id,
            kind,
            look: kind.look(),
            group: group.to_string(),
            tab: tab.to_string(),
            title: String::new(),
            detail: String::new(),
            at: 0,
            buttons: vec![],
            unseen: false,
        }
    }

    pub(in crate::desk::center) fn says(mut self, title: &str, detail: String) -> Self {
        self.title = title.to_string();
        self.detail = detail;
        self
    }

    pub(in crate::desk::center) fn at(mut self, at: u64) -> Self {
        self.at = at;
        self
    }

    pub(in crate::desk::center) fn button(mut self, label: &'static str, does: Do) -> Self {
        let role = does.role();
        self.buttons.push(Button { label, role, does });
        self
    }
}
