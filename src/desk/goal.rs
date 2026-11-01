//! A `goal` on a tab: the decision model carries out a screen's worth of clicks or keys on the
//! tab's view, and the report says what it did, step by step, and how it ended.

use std::fmt::Write as _;
use std::time::Duration;

use crate::goal::decision_loop::DecisionLoop;
use crate::hands::{Hands, Outcome, Report, Task};
use crate::model::Decider;
use crate::openrouter::OpenRouter;
use crate::secrets::Vault;
use crate::view::View;

use super::sealed::Sealed;
use super::{Call, Failure};

/// Actions one goal may take.
const MAX_STEPS: usize = 40;

/// `goal` carried out on `view`, typing only `values`; a value that names a saved secret
/// (`{{secret:NAME}}`) is typed as the secret, which neither the decision model nor any answer
/// sees.
pub(super) fn run_goal(
    call: &Call,
    tab: &str,
    view: &dyn View,
    goal: &str,
    values: &[String],
    vault: &Vault,
    paid: &mut Vec<String>,
) -> Result<String, Failure> {
    let dir = crate::secrets::dir();
    for value in values {
        vault.fill_in(dir.as_deref(), value)?;
    }
    // Every secret the daemon knows, not only the goal's: one an earlier call typed may still be
    // on the screen. Only the goal's values name what it types.
    let view = Sealed {
        view,
        secrets: vault.known(),
        paid: paid.clone(),
    };
    let key = call
        .key
        .map(String::from)
        .or_else(crate::key::find)
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            Failure::Failed(
                "a goal needs the decision model: save the key of an OpenRouter account once \
                     with `llx-desk key` (it reads it from stdin), or set OPENROUTER_API_KEY; \
                     everything else works without one"
                    .into(),
            )
        })?;
    let model = std::env::var("DESK_DECISION_MODEL").ok();
    let decider = OpenRouter::new(key.trim(), model.as_deref());
    drive(call, tab, &view, &decider, goal, values, paid)
}

/// `goal` carried out on `view` by `decider`, each step told to the call's progress.
fn drive(
    call: &Call,
    tab: &str,
    view: &dyn View,
    decider: &dyn Decider,
    goal: &str,
    values: &[String],
    paid: &mut Vec<String>,
) -> Result<String, Failure> {
    let hands = DecisionLoop {
        decider,
        on_step: &|step| {
            if let Some(progress) = call.progress {
                progress(tab, &step.trace_line());
            }
        },
        pause: &pause,
        max_actions: MAX_STEPS,
    };
    let done = hands.carry_out(view, Task { goal, values });
    for control in &done.paid {
        if !paid.contains(control) {
            paid.push(control.clone());
        }
    }
    if let Outcome::Unreachable(why) = &done.outcome {
        return Err(Failure::Fatal(why.clone()));
    }
    Ok(format!("{tab}: {}", helper(&done, values)))
}

#[expect(
    clippy::disallowed_methods,
    reason = "a page has no change event: the goal looks again after a short pause it chose"
)]
fn pause(wait: Duration) {
    std::thread::sleep(wait);
}

/// What the helper did, step by step, and how it ended; the values it never typed.
fn helper(done: &Report, values: &[String]) -> String {
    let outcome = match &done.outcome {
        Outcome::Done { probability } => format!("done ({:.0}% sure)", probability * 100.0),
        Outcome::Blocked => "blocked: nothing in the tab could make progress".into(),
        Outcome::Exhausted => "out of actions".into(),
        Outcome::Stopped(why) | Outcome::Unreachable(why) => format!("stopped: {why}"),
    };
    let mut out = format!("### Helper: {outcome}");
    for (i, step) in done.steps.iter().enumerate() {
        let _ = write!(out, "\n{}. {step}", i + 1);
    }
    let typed = |value: &String| {
        done.steps
            .iter()
            .any(|s| s.contains(&format!("\"{value}\"")))
    };
    let unused: Vec<String> = (values.iter())
        .filter(|value| !typed(value))
        .map(|value| format!("\"{value}\""))
        .collect();
    if !unused.is_empty() {
        let _ = write!(out, "\nValues never typed: {}.", unused.join(", "));
    }
    if !done.commits.is_empty() {
        let _ = write!(
            out,
            "\nIt clicked what commits: {}. Read the page before doing any of it again.",
            done.commits.join("; ")
        );
    }
    let _ = write!(
        out,
        "\nModel: {} decisions, ${:.4}, {:.1} s.",
        done.decisions,
        done.cost,
        Duration::from_millis(done.model_millis).as_secs_f64()
    );
    for line in held_back(&done.withheld) {
        out.push('\n');
        out.push_str(&line);
    }
    if !done.withheld.is_empty() {
        out.push_str(" A goal pays, orders or deletes only when its words ask to.");
    }
    out
}

/// The most controls named for one reason; the rest are counted.
const NAMED: usize = 3;

/// The controls held back (`control: why`), one line per reason.
fn held_back(withheld: &[String]) -> Vec<String> {
    let mut by_why: Vec<(&str, Vec<&str>)> = vec![];
    for line in withheld {
        let (control, why) = line.rsplit_once(": ").unwrap_or((line, "held back"));
        match by_why.iter_mut().find(|(w, _)| *w == why) {
            Some((_, controls)) => controls.push(control),
            None => by_why.push((why, vec![control])),
        }
    }
    (by_why.into_iter())
        .map(|(why, controls)| {
            let named = controls[..controls.len().min(NAMED)].join(", ");
            let more = match controls.len().saturating_sub(NAMED) {
                0 => String::new(),
                n => format!(" and {n} more"),
            };
            format!("Held back from it, as {why}: {named}{more}.")
        })
        .collect()
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod tests;
