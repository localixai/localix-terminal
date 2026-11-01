//! A terminal session as a view, for a `goal`: its screen is the page, its one control a text
//! field that types into the program.

use crate::view::{Ability, Action, Element, Observation, View, ViewError, ViewResult};

use super::Session;

const FIELD: &str = "terminal";
const CONVENTIONS: &str = "A terminal: text typed into the terminal field goes to the program; \
pressing Enter submits the line; keys (arrows, Escape, Tab) go to the program as pressed. The \
screen shows what the program drew.";

pub struct TerminalView<'a>(pub &'a Session);

impl View for TerminalView<'_> {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        if self.0.ended() {
            return Err(ViewError::Unavailable("the program has ended".into()));
        }
        let screen = self.0.snapshot();
        let version = self.0.version();
        Ok(Observation {
            epoch: String::new(),
            location: self.0.command().to_string(),
            title: format!("terminal · {}", self.0.command()),
            text: screen.clone(),
            outline: format!("{screen}\n- textbox \"terminal\" [ref={FIELD}]"),
            elements: vec![Element {
                reference: FIELD.into(),
                role: "textbox".into(),
                name: "terminal".into(),
                value: None,
                states: vec![],
                editable: true,
                dialog: false,
                options: vec![],
                form: None,
            }],
            busy: false,
            scroll: 0,
            revision: version,
            notices: vec![],
        })
    }

    fn act(&self, _epoch: &str, action: &Action) -> ViewResult<()> {
        let refused = |why: &str| Err(ViewError::Refused(why.into()));
        match action {
            Action::Type { text, .. } => self.0.type_in(text, &[]).map_err(ViewError::Unavailable),
            Action::PressEnter { .. } => self
                .0
                .type_in("", &["Enter".into()])
                .map_err(ViewError::Unavailable),
            Action::Click { .. } => {
                refused("a terminal has the field to type into, nothing to click")
            }
            Action::Select { .. } => refused("a terminal has no menus"),
            Action::ScrollDown | Action::ScrollUp => refused("a terminal does not scroll"),
            // The key as a call's `keys` send it.
            Action::Press { key } => self
                .0
                .type_in("", std::slice::from_ref(key))
                .map_err(ViewError::Refused),
            Action::Go { .. } => refused("a terminal has no address to go to"),
        }
    }

    fn abilities(&self) -> &'static [Ability] {
        &[Ability::Keys]
    }
}
