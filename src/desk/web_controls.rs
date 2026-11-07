//! Control refs decoded and applied in order against the page's current document.

use crate::browser::Page;
use crate::view::{Action, View};
use serde_json::Value;

/// The call's acts on the page's controls by their refs, in order: fields filled, options
/// selected, then what is clicked. Each acts on the page as it is just then, so a click that
/// loads another page is the last that lands; a ref that is gone or covered is said, and the
/// rest are not done.
pub(super) fn apply(page: &Page, args: &Value, report: &mut Vec<String>) -> Result<(), String> {
    let pairs = |field: &str| -> Vec<(String, String)> {
        (args
            .get(field)
            .and_then(Value::as_object)
            .into_iter()
            .flatten())
        .filter_map(|(at, value)| Some((at.clone(), value.as_str()?.to_string())))
        .collect()
    };
    let mut acts: Vec<(String, Action)> = vec![];
    for (at, text) in pairs("fill") {
        let reference = control(&at)?;
        acts.push((
            format!("Filled {reference}."),
            Action::Type { reference, text },
        ));
    }
    for (at, value) in pairs("select") {
        let reference = control(&at)?;
        let said = format!("Selected \"{value}\" in {reference}.");
        acts.push((said, Action::Select { reference, value }));
    }
    let clicks = match args.get("click") {
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => many
            .iter()
            .filter_map(|at| at.as_str().map(String::from))
            .collect(),
        _ => vec![],
    };
    for at in clicks {
        let reference = control(&at)?;
        acts.push((format!("Clicked {reference}."), Action::Click { reference }));
    }
    for (said, action) in acts {
        let epoch = page.epoch().map_err(|e| e.to_string())?;
        match page.act(&epoch, &action) {
            Ok(()) => report.push(said),
            Err(why) => {
                report.push(format!("{} not done: {why}", said.trim_end_matches('.')));
                break;
            }
        }
    }
    Ok(())
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
