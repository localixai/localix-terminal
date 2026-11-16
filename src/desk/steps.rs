//! Several steps in one call (`{"action": "steps"}`): each an action's object or a short
//! command's string (`crate::short`, the command line's own table). Every step is checked before
//! the first runs; they run in order, each on the tab of its kind the step before used; the first
//! that fails or misses stops the rest. The answer is a line a step and, once, the tab of the last
//! step run, told against what the caller last read: a step's own reads are not the caller's.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::actions::parse;
use super::args::{check, text};
use super::changes::Seen;
use super::tab::unread;
use super::{Call, Desk, Failure, Output};

/// The most steps one call runs.
const MOST_STEPS: usize = 16;
/// How long the steps may take: one that would start later is not run.
const MOST_TIME: Duration = Duration::from_secs(60);
/// The actions a step may be.
const ACTIONS: [&str; 7] = [
    "open",
    "interact",
    "screenshot",
    "close",
    "show",
    "list",
    "guide",
];
/// The fields of `interact` only a page takes: a step with one works in a page.
const PAGE_ONLY: [&str; 11] = [
    "url",
    "click",
    "fill",
    "select",
    "script",
    "read",
    "offset",
    "size",
    "key_delay_ms",
    "dialog",
    "dialog_text",
];

impl Desk {
    /// The call's `steps`, run in order until one fails or misses.
    pub(super) fn steps(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let given = (args.get("steps").and_then(Value::as_array)).map_or(&[][..], Vec::as_slice);
        let steps = self.planned(given)?;
        let total = steps.len();
        let started = Instant::now();
        let seen = self.seen_by(call.owner);
        let mut lines: Vec<String> = vec![];
        let mut before = Output::default();
        for (at, step) in steps.iter().enumerate() {
            let number = at + 1;
            if at > 0 && started.elapsed() > MOST_TIME {
                lines.push(format!("The call's 60 s ran out. {}", stopped(at, total)));
                return Ok(Output {
                    report: lines.join("\n"),
                    missed: true,
                    ..before
                });
            }
            if at > 0 {
                self.seen_again(call.owner, &seen);
            }
            let done = (self.with_tab(call, step).map_err(Failure::from))
                .and_then(|step| self.act(call, &step));
            match done {
                Ok(out) if total == 1 => return Ok(out),
                Err(why) if total == 1 => return Err(why),
                Ok(out) if number == total || out.missed => {
                    lines.push(format!("{number}. {}", out.report));
                    if number < total {
                        lines.push(stopped(number, total));
                    }
                    return Ok(Output {
                        report: lines.join("\n"),
                        ..out
                    });
                }
                Ok(out) => {
                    lines.push(format!("{number}. {}", out.report));
                    before = Output {
                        report: String::new(),
                        shot: None,
                        ..out
                    };
                }
                Err(why) => {
                    lines.push(format!("{number}. {}", why.message()));
                    lines.push(stopped(number, total));
                    return Err(lines.join("\n").into());
                }
            }
        }
        Err("`steps` held no step: nothing was done".to_string().into())
    }

    /// Every step as an action, each checked: a step that is wrong is refused before any runs.
    fn planned(&self, given: &[Value]) -> Result<Vec<Value>, String> {
        if given.is_empty() {
            return Err(
                "`steps` is a list of steps, each an action's object or a command's \
                        string (`click e12`): nothing was done"
                    .into(),
            );
        }
        if given.len() > MOST_STEPS {
            return Err(format!(
                "a call runs at most {MOST_STEPS} steps, not {}: nothing was done",
                given.len()
            ));
        }
        let last = given.len();
        (given.iter().enumerate())
            .map(|(at, step)| {
                let said = |why: &str| format!("step {}: {why}", at + 1);
                let step = match step {
                    Value::String(line) => crate::short::line(line)
                        .map_err(|why| said(&format!("{why}; nothing was done")))?,
                    Value::Object(_) => step.clone(),
                    other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_)) => {
                        return Err(said(&format!(
                            "a step is an action's object or a command's string, not {other}; \
                             nothing was done"
                        )));
                    }
                };
                let action = text(&step, "action").unwrap_or_default();
                if !ACTIONS.contains(&action) {
                    let why = format!("a step's `action` is one of {}", ACTIONS.join(", "));
                    return Err(said(&format!("{why}, not `{action}`; nothing was done")));
                }
                if action == "screenshot" && at + 1 != last {
                    return Err(said("a screenshot is the last step only; nothing was done"));
                }
                check(&step).map_err(|why| said(&why))?;
                let tab = text(&step, "tab").and_then(|key| parse(key, &self.made.kinds));
                if let (Some(tab), "interact") = (tab, action) {
                    unread(tab.kind, &step).map_err(|why| said(&why))?;
                }
                Ok(step)
            })
            .collect()
    }

    /// `step` with the tab it works in: the one it names, else the one of its kind its caller
    /// worked in last (the step before's, among steps).
    fn with_tab(&self, call: &Call, step: &Value) -> Result<Value, String> {
        let tabbed = match text(step, "action") {
            // With no tab, an ask or a session's id is the caller's session's own.
            Some("interact") => step.get("ask").is_none() && step.get("session").is_none(),
            Some("screenshot") => true,
            _ => false,
        };
        if !tabbed || text(step, "tab").is_some() {
            return Ok(step.clone());
        }
        let letter = match (
            PAGE_ONLY.iter().any(|field| step.get(*field).is_some()),
            step.get("input"),
        ) {
            (true, _) => Some('w'),
            (false, Some(_)) => Some('t'),
            (false, None) => None,
        };
        let last = crate::lock(&self.registry).last_used(call.owner, letter);
        let key = last.ok_or_else(|| match letter {
            Some('w') => "you have no page open: `llxterm open URL` opens one".to_string(),
            Some(_) => "you have no terminal open: `llxterm term [CMD]` opens one".to_string(),
            None => "you have no tab open: `open` one first".to_string(),
        })?;
        let mut step = step.clone();
        step["tab"] = Value::String(key);
        Ok(step)
    }

    /// What `owner` has read of its pages, as it is now.
    fn seen_by(&self, owner: &str) -> HashMap<u32, Seen> {
        let held: Vec<(u32, Arc<Mutex<Seen>>)> = {
            let seen = crate::lock(&self.seen);
            (self.pages_of(owner).into_iter())
                .filter_map(|id| Some((id, Arc::clone(seen.get(&id)?))))
                .collect()
        };
        (held.into_iter())
            .map(|(id, page)| (id, crate::lock(&page).clone()))
            .collect()
    }

    /// `owner`'s pages known again as `before` had them: one it has not read is not known.
    fn seen_again(&self, owner: &str, before: &HashMap<u32, Seen>) {
        let pages = self.pages_of(owner);
        let mut seen = crate::lock(&self.seen);
        for id in pages {
            match before.get(&id) {
                Some(was) => seen.insert(id, Arc::new(Mutex::new(was.clone()))),
                None => seen.remove(&id),
            };
        }
    }

    /// The ids of `owner`'s pages.
    fn pages_of(&self, owner: &str) -> Vec<u32> {
        let keys = crate::lock(&self.registry).tabs_of(owner);
        (keys.iter())
            .filter_map(|key| key.strip_prefix('w')?.parse().ok())
            .collect()
    }
}

/// The line that says the steps after step `at` of `total` were not done.
fn stopped(at: usize, total: usize) -> String {
    let left = match total - at {
        1 => format!("step {total} was not done"),
        _ => format!("steps {}-{total} were not done", at + 1),
    };
    format!("Stopped at step {at} of {total}: {left}.")
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
