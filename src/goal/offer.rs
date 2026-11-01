//! What a step can offer: the controls in scope (a dialog's alone while one is open), the
//! operations they allow now, the (field, value) pairs worth typing, and the targets the view
//! refused as covered.

use serde_json::{Map, Value, json};

use crate::view::{Ability, Element, SelectOption};

use crate::goal::step::{GoalStep, OPERATIONS, Operation};

pub(crate) fn clickable(element: &Element) -> bool {
    !element.disabled()
}

fn editable(element: &Element) -> bool {
    clickable(element) && element.editable
}

pub(crate) fn dropdown(element: &Element) -> bool {
    clickable(element) && choices(element).next().is_some()
}

/// A dropdown's options that choosing would change, each with its place among them all: the one
/// it shows already is no step.
pub(crate) fn choices(element: &Element) -> impl Iterator<Item = (usize, &SelectOption)> {
    let shown = element.value.as_deref();
    (element.options.iter().enumerate())
        .filter(move |(_, option)| Some(option.label.as_str()) != shown)
}

/// While a dialog is open, only its controls are offered: it blocks the rest of the view.
pub(crate) fn in_scope(elements: &[Element]) -> Vec<&Element> {
    let dialog: Vec<&Element> = elements.iter().filter(|element| element.dialog).collect();
    if dialog.is_empty() {
        elements.iter().collect()
    } else {
        dialog
    }
}

/// What a view takes besides its controls, as a step can use it: keys, and the supplied values
/// that are addresses where it takes one.
pub(crate) struct Beyond<'a> {
    pub(crate) keys: bool,
    pub(crate) addresses: Vec<&'a str>,
}

impl<'a> Beyond<'a> {
    pub(crate) fn of(abilities: &[Ability], values: &'a [String]) -> Self {
        let addresses = match abilities.contains(&Ability::Address) {
            true => values
                .iter()
                .map(String::as_str)
                .filter(|value| address(value))
                .collect(),
            false => vec![],
        };
        Self {
            keys: abilities.contains(&Ability::Keys),
            addresses,
        }
    }
}

/// A value the view may go to: an http or https address, whole.
fn address(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")) && !lower.contains(' ')
}

/// The operations available in this view, now.
pub(crate) fn operations_now(
    elements: &[&Element],
    typing: &[(String, String, String)],
    history: &[GoalStep],
    busy: bool,
    beyond: &Beyond,
) -> Vec<Operation> {
    let last = history.last();
    OPERATIONS
        .into_iter()
        .filter(|operation| match operation {
            Operation::Click => elements.iter().any(|e| clickable(e)),
            Operation::TypeText => !typing.is_empty(),
            Operation::Select => elements.iter().any(|e| dropdown(e)),
            Operation::PressEnter => last.is_some_and(|step| step.operation == Operation::TypeText),
            // Waiting helps while the view says it is working, or right after an action that has
            // shown nothing yet (a slow answer). On a settled view a WAIT only defers DONE: after
            // "click Next and wait for the next question" it waited, then answered more.
            Operation::Wait => {
                busy || last.is_some_and(|step| {
                    step.operation != Operation::Wait && step.changed == Some(false)
                })
            }
            // A scroll that moved nothing is not offered again that way; three in a row end it.
            Operation::ScrollDown | Operation::ScrollUp => {
                let repeated = last.is_some_and(|step| {
                    step.operation == *operation && step.changed == Some(false)
                });
                let recent = &history[history.len().saturating_sub(3)..];
                let scrolling =
                    recent.len() == 3 && recent.iter().all(|step| step.operation.scroll());
                !(repeated || scrolling)
            }
            Operation::PressKey => beyond.keys,
            Operation::GoTo => !beyond.addresses.is_empty(),
            Operation::Done | Operation::Blocked => true,
        })
        .collect()
}

pub(crate) fn criteria<'a>(elements: impl Iterator<Item = &'a &'a Element>) -> Map<String, Value> {
    elements
        .map(|element| (element.reference.clone(), json!(element.describe())))
        .collect()
}

/// Every (field, value) pair worth typing, as one choice: a field and a value chosen apart can
/// pair a city with a date field. A field that holds one of the values is filled, and a value a
/// field holds is placed: neither is offered again, so a filled form never trades values between
/// its fields (name into e-mail, e-mail into name), except that a field the page refused takes
/// a value placed nowhere yet. A password field reads as dots: it holds what
/// this run last typed into the field of its name.
pub(crate) fn entries(
    elements: &[&Element],
    values: &[String],
    history: &[GoalStep],
) -> Vec<(String, String, String)> {
    let holds = |element: &Element, value: &str| match element.value.as_deref() {
        Some(dots) if !dots.is_empty() && dots.chars().all(|c| c == '•') => {
            typed_into(history, element) == Some(value)
        }
        Some(now) => now.trim().eq_ignore_ascii_case(value.trim()),
        None => false,
    };
    let editable: Vec<&Element> = elements.iter().copied().filter(|e| editable(e)).collect();
    let placed = |value: &str| editable.iter().any(|e| holds(e, value));
    // A field the page refused (`[invalid: …]`) takes another value, never the one it refused.
    let refused = |element: &Element| element.states.iter().any(|s| s.starts_with("invalid"));
    let mut out = vec![];
    for element in editable
        .iter()
        .filter(|e| refused(e) || !values.iter().any(|v| holds(e, v)))
    {
        for (i, value) in values.iter().enumerate() {
            if !holds(element, value) && !placed(value) && out.len() < MAX_ENTRIES {
                let key = format!("{}=v{}", element.reference, i + 1);
                let text = format!("{} ← \"{value}\"", element.describe());
                out.push((key, element.reference.clone(), text));
            }
        }
    }
    out
}

const MAX_ENTRIES: usize = 80;

/// What this run last typed into the field `element` is, by its role and name.
fn typed_into<'a>(history: &'a [GoalStep], element: &Element) -> Option<&'a str> {
    let label = element.label();
    (history.iter().rev())
        .find(|step| {
            step.operation == Operation::TypeText
                && step.failed.is_none()
                && step.control.as_deref() == Some(label.as_str())
        })
        .and_then(|step| step.value.as_deref())
}

/// The targets the view refused as covered since it last changed: clicking them again only fails
/// again, so the decision goes to what covers them (a dialog the next look offers) instead, as
/// long as something else is offered.
pub(crate) fn covered(history: &[GoalStep]) -> Vec<&str> {
    history
        .iter()
        .rev()
        .take_while(|step| step.changed != Some(true))
        .filter(|step| {
            step.failed
                .as_deref()
                .is_some_and(|e| e.starts_with("covered"))
        })
        .filter_map(|step| step.reference.as_deref())
        .collect()
}
