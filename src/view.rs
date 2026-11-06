//! The view protocol: what a view shows and what can be done to it. A view shows controls by refs
//! and is acted on by a ref and an operation, never a selector or coordinates. A view knows nothing
//! of who drives it.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Something an agent can see and act on by refs: a document, a screen.
pub trait View {
    fn observe(&self) -> ViewResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the view settle.
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()>;
}

/// The keys one names by name, the same everywhere: the `keys` of a call, what a page and a terminal take (`Ctrl+<letter>` and one character too, in a call).
pub const KEYS: [&str; 15] = [
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Enter",
    "Escape",
    "Tab",
    "Shift+Tab",
    "Space",
    "Backspace",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

/// One atomic read of what the view shows: the controls one can see and use, and the text around.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Observation {
    /// Changes when the view shows a new document or screen; an action carries it so it never
    /// lands on a newer one.
    pub epoch: String,
    /// Where the view is, in its own terms (an address, a path, a screen).
    pub location: String,
    pub title: String,
    pub text: String,
    /// The view as an outline: headings and text in reading order, every control with its ref,
    /// indented by structure. Empty from a view that has none.
    #[serde(default)]
    pub outline: String,
    pub elements: Vec<Element>,
    /// The view says it is still working (loading, fetching, a progress bar, "Loading…").
    #[serde(default)]
    pub busy: bool,
    /// How far the view is scrolled: a scroll that moved nothing is told apart from one that did.
    #[serde(default)]
    pub scroll: i64,
    /// How many times the view has changed anywhere, in sight or out of it (a counter scrolled
    /// above, a message shown again in the same words): an action that changed only that still
    /// changed something.
    #[serde(default)]
    pub revision: u64,
    /// What the view told since the last action, besides what it shows: a dialog and how it was
    /// answered, words announced and gone, a field that did not keep what was typed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Element {
    #[serde(rename = "ref")]
    pub reference: String,
    pub role: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub editable: bool,
    /// In a dialog that covers the rest: while one is open, only its controls are offered.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dialog: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SelectOption>,
    /// The form it is in, by a name the probe gives each (`f1`): Enter in a field sends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

impl Element {
    /// `textbox "Where to?" = "London" [expanded]`: how an element reads in requests and traces.
    pub fn describe(&self) -> String {
        let mut line = self.role.clone();
        if !self.name.is_empty() {
            let _ = write!(line, " \"{}\"", self.name);
        }
        if let Some(value) = self.value.as_deref().filter(|value| !value.is_empty()) {
            let _ = write!(line, " = \"{value}\"");
        }
        if !self.states.is_empty() {
            let _ = write!(line, " [{}]", self.states.join(", "));
        }
        line
    }

    /// `button "Pay"`: what the element is, without its value and states, which change.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            self.role.clone()
        } else {
            format!("{} \"{}\"", self.role, self.name)
        }
    }

    pub fn disabled(&self) -> bool {
        self.states.iter().any(|state| state == "disabled")
    }
}

/// One operation on the view. Targets are refs from the latest observation.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click {
        reference: String,
    },
    Type {
        reference: String,
        text: String,
    },
    Select {
        reference: String,
        value: String,
    },
    PressEnter {
        reference: Option<String>,
    },
    ScrollDown,
    ScrollUp,
    /// One of `KEYS`, pressed where the focus is, as a call's `keys` press it.
    Press {
        key: String,
    },
    /// An address opened in the view, as a call's `url` opens it.
    Go {
        url: String,
    },
}

/// Why the view could not do what was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    /// What the action named changed since the observation; observe again.
    Stale(String),
    /// The view refused the action (covered, disabled, not editable).
    Refused(String),
    /// The view did not answer in time, or is gone.
    Unavailable(String),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale(message) | Self::Refused(message) | Self::Unavailable(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for ViewError {}

pub type ViewResult<T> = Result<T, ViewError>;
