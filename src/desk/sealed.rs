//! A goal's view, sealed: its decision model never reads a secret or a credential, and still has
//! one typed. What the view shows is masked (a saved secret's value as `{{secret:NAME}}`, a
//! credential's shape as `[credential]`), so the model reads a field holding a secret as holding
//! the value it was given; a value typed that names a secret has the secret put in, there only.

use crate::secrets::reference;
use crate::view::{Ability, Action, Element, Observation, SelectOption, View, ViewResult};

pub(super) struct Sealed<'a> {
    pub(super) view: &'a dyn View,
    /// The secrets the daemon knows, by name: masked wherever they show, put in where a value
    /// the goal types names one.
    pub(super) secrets: Vec<(String, String)>,
    /// The paying controls earlier goals in this tab clicked.
    pub(super) paid: Vec<String>,
}

impl Sealed<'_> {
    fn mask(&self, text: &str) -> String {
        crate::shapes::mask(&crate::secrets::masked(&self.secrets, text))
    }

    fn filled(&self, text: &str) -> String {
        let mut filled = text.to_string();
        for (name, value) in &self.secrets {
            filled = filled.replace(&reference(name), value);
        }
        filled
    }

    fn masked(&self, view: Observation) -> Observation {
        let element = |element: Element| Element {
            name: self.mask(&element.name),
            value: element.value.as_deref().map(|value| self.mask(value)),
            states: element
                .states
                .iter()
                .map(|state| self.mask(state))
                .collect(),
            // An option's label is what the model reads; its value is only what is selected.
            options: (element.options.into_iter())
                .map(|option| SelectOption {
                    label: self.mask(&option.label),
                    ..option
                })
                .collect(),
            ..element
        };
        Observation {
            location: self.mask(&view.location),
            title: self.mask(&view.title),
            text: self.mask(&view.text),
            outline: self.mask(&view.outline),
            elements: view.elements.into_iter().map(element).collect(),
            notices: view
                .notices
                .iter()
                .map(|notice| self.mask(notice))
                .collect(),
            ..view
        }
    }
}

impl View for Sealed<'_> {
    fn conventions(&self) -> &str {
        self.view.conventions()
    }

    fn paid(&self) -> Vec<String> {
        self.paid.clone()
    }

    fn observe(&self) -> ViewResult<Observation> {
        self.view.observe().map(|view| self.masked(view))
    }

    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
        match action {
            Action::Type { reference, text } => self.view.act(
                epoch,
                &Action::Type {
                    reference: reference.clone(),
                    text: self.filled(text),
                },
            ),
            Action::Click { .. }
            | Action::Select { .. }
            | Action::PressEnter { .. }
            | Action::ScrollDown
            | Action::ScrollUp
            | Action::Press { .. }
            | Action::Go { .. } => self.view.act(epoch, action),
        }
    }

    fn abilities(&self) -> &'static [Ability] {
        self.view.abilities()
    }
}

#[cfg(test)]
#[path = "sealed_tests.rs"]
mod tests;
