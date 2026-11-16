//! Controls named by a call, by ref or by exact name, applied in order against the page's current
//! document.

use crate::browser::Page;
use crate::view::{Action, Element, View};
use serde_json::Value;

/// What one act is on: as the call named it, and what it does once that control is found.
struct Act {
    at: String,
    verb: Verb,
}

#[derive(Clone)]
enum Verb {
    Fill(String),
    Select(String),
    Click,
}

/// The call's acts on the page's controls, in order: fields filled, options selected, then what
/// is clicked. Each acts on the page as it is just then, so a click that loads another page is the
/// last that lands, and a control named by its name is found on that page. A control that is
/// gone, covered or not found is said, and the rest are not done: then this is false.
pub(super) fn apply(page: &Page, args: &Value, report: &mut Vec<String>) -> Result<bool, String> {
    let pairs = |field: &str| -> Vec<(String, String)> {
        (args
            .get(field)
            .and_then(Value::as_object)
            .into_iter()
            .flatten())
        .filter_map(|(at, value)| Some((at.clone(), value.as_str()?.to_string())))
        .collect()
    };
    let clicks = match args.get("click") {
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => many
            .iter()
            .filter_map(|at| at.as_str().map(String::from))
            .collect(),
        _ => vec![],
    };
    let acts: Vec<Act> = (pairs("fill").into_iter())
        .map(|(at, text)| Act {
            at,
            verb: Verb::Fill(text),
        })
        .chain((pairs("select").into_iter()).map(|(at, value)| Act {
            at,
            verb: Verb::Select(value),
        }))
        .chain(clicks.into_iter().map(|at| Act {
            at,
            verb: Verb::Click,
        }))
        .collect();
    // Nothing is done when one of them names no control at all.
    if let Some(empty) = acts.iter().find(|act| act.at.trim().is_empty()) {
        return Err(format!(
            "`{}` is not a control's ref or name: the outline names them, `[e12]`",
            empty.at
        ));
    }
    for act in acts {
        let (found, epoch) = match control(&act.at) {
            Ok(reference) => (Ok(reference), page.epoch().map_err(|e| e.to_string())?),
            Err(_) => {
                let seen = page.observe().map_err(|e| e.to_string())?;
                (named(&seen.elements, act.at.trim()), seen.epoch)
            }
        };
        let not_done = |reference: &str, why: &dyn std::fmt::Display| {
            format!(
                "{} not done: {why}",
                said(&act, reference).trim_end_matches('.')
            )
        };
        let reference = match found {
            Ok(reference) => reference,
            Err(why) => {
                report.push(not_done(act.at.trim(), &why));
                return Ok(false);
            }
        };
        let action = match act.verb.clone() {
            Verb::Fill(text) => Action::Type {
                reference: reference.clone(),
                text,
            },
            Verb::Select(value) => Action::Select {
                reference: reference.clone(),
                value,
            },
            Verb::Click => Action::Click {
                reference: reference.clone(),
            },
        };
        if let Err(why) = page.act(&epoch, &action) {
            report.push(not_done(&reference, &why));
            return Ok(false);
        }
        report.push(said(&act, &reference));
    }
    Ok(true)
}

/// How a done act is said: by its ref, and by the name it was found by.
fn said(act: &Act, reference: &str) -> String {
    let at = match control(&act.at) {
        Ok(_) => reference.to_string(),
        Err(_) if act.at.trim() == reference => format!("\"{reference}\""),
        Err(_) => format!("\"{}\" ({reference})", act.at.trim()),
    };
    match &act.verb {
        Verb::Fill(_) => format!("Filled {at}."),
        Verb::Select(value) => format!("Selected \"{value}\" in {at}."),
        Verb::Click => format!("Clicked {at}."),
    }
}

/// A control's ref as a call gives it: `e12`, `@e12` or `[e12]`; or one of the browser's own
/// (`back`, `reload`), which the outline lists as it does the page's.
pub(super) fn control(at: &str) -> Result<String, String> {
    let at = at
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('[')
        .trim_end_matches(']');
    if crate::browser::owns(at) {
        return Ok(at.to_string());
    }
    let digits = at
        .strip_prefix('e')
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    digits
        .map(|n| format!("e{n}"))
        .ok_or_else(|| format!("`{at}` is not a control's ref: the outline names them, `[e12]`"))
}

/// The one control of `elements` whose name is `name`, exactly, else when nothing has it exactly
/// in another case; while a dialog covers the page, only among its controls. None or several:
/// what there is instead, so the agent picks by ref.
pub(super) fn named(elements: &[Element], name: &str) -> Result<String, String> {
    let covered = elements.iter().any(|element| element.dialog);
    let open: Vec<&Element> = (elements.iter())
        .filter(|element| !covered || element.dialog)
        .collect();
    let mut found: Vec<&&Element> = open.iter().filter(|element| element.name == name).collect();
    if found.is_empty() {
        found = (open.iter())
            .filter(|element| element.name.eq_ignore_ascii_case(name))
            .collect();
    }
    match found.as_slice() {
        [one] => Ok(one.reference.clone()),
        [] => {
            let lower = name.to_lowercase();
            let near: Vec<String> = (open.iter())
                .filter(|element| !element.name.is_empty())
                .filter(|element| element.name.to_lowercase().contains(&lower))
                .take(5)
                .map(|element| format!("\"{}\" [{}]", element.name, element.reference))
                .collect();
            let instead = match near.is_empty() {
                true => "read the page for its refs".to_string(),
                false => format!("the closest: {}", near.join(", ")),
            };
            Err(format!("no control is named \"{name}\"; {instead}"))
        }
        many => {
            let refs: Vec<&str> = many
                .iter()
                .map(|element| element.reference.as_str())
                .collect();
            Err(format!(
                "{} controls are named \"{name}\" ({}): name one by its ref",
                many.len(),
                refs.join(", ")
            ))
        }
    }
}
