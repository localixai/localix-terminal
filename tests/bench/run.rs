//! One benchmark task, run once through the desk as an agent's calls would: open the tab, set it
//! up, check that it is not done already, hand each goal to the helper, then check the outcome
//! with the task's own expression on the page (or text on a terminal's screen).

use std::path::Path;
use std::time::Instant;

use llx_desk::desk::{Call, Desk};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub(crate) struct Task {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) safety: bool,
    pub(crate) page: Option<String>,
    pub(crate) terminal: Option<String>,
    pub(crate) setup: Option<String>,
    pub(crate) goals: Vec<Goal>,
    pub(crate) check: Option<String>,
    pub(crate) screen: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Goal {
    goal: String,
    #[serde(default)]
    values: Vec<String>,
}

/// What one run came to.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Run {
    pub(crate) task: String,
    pub(crate) repeat: usize,
    pub(crate) safety: bool,
    /// The check held after the goals.
    pub(crate) passed: bool,
    /// The check did not hold before them: a task that a run can fail.
    pub(crate) valid: bool,
    /// The helper ended its last goal saying it was done.
    pub(crate) claimed_done: bool,
    /// How each goal ended, in the helper's words.
    pub(crate) outcomes: Vec<String>,
    /// The helper's steps, as its report lists them: what to read when a run fails.
    pub(crate) trace: Vec<String>,
    /// The helper's steps (its turns), DONE included.
    pub(crate) steps: usize,
    /// Requests to the decision model.
    pub(crate) decisions: usize,
    pub(crate) cost: f64,
    pub(crate) model_s: f64,
    pub(crate) wall_s: f64,
    pub(crate) error: Option<String>,
}

pub(crate) fn run(desk: &Desk, call: &Call, task: &Task, repeat: usize) -> Run {
    let started = Instant::now();
    let mut run = Run {
        task: task.id.clone(),
        repeat,
        safety: task.safety,
        ..Run::default()
    };
    if let Err(error) = carry(desk, call, task, &mut run) {
        run.error = Some(error);
    }
    run.wall_s = started.elapsed().as_secs_f64();
    run
}

fn carry(desk: &Desk, call: &Call, task: &Task, run: &mut Run) -> Result<(), String> {
    let opened = match (&task.page, &task.terminal) {
        (Some(page), _) => json!({"action": "open", "kind": "browser", "url": page}),
        (None, Some(command)) => json!({"action": "open", "kind": "terminal", "command": command}),
        (None, None) => return Err("a task has a page or a terminal".into()),
    };
    let answer = window(desk, call, &opened)?;
    let tab = answer
        .split_whitespace()
        .next()
        .ok_or("no tab opened")?
        .to_string();
    let done = (|| {
        if let Some(setup) = &task.setup {
            script(desk, call, &tab, setup)?;
        }
        run.valid = !holds(desk, call, &tab, task)?;
        if !run.valid {
            return Err("the check holds before any goal: the task proves nothing".into());
        }
        for goal in &task.goals {
            let args =
                json!({"action": "interact", "tab": tab, "goal": goal.goal, "values": goal.values});
            let answer = window(desk, call, &args)?;
            helper(&answer, run);
        }
        run.passed = holds(desk, call, &tab, task)?;
        Ok(())
    })();
    let _ = window(desk, call, &json!({"action": "close", "tab": tab}));
    done
}

fn window(desk: &Desk, call: &Call, args: &Value) -> Result<String, String> {
    desk.window(call, args).map_err(|e| e.message().to_string())
}

/// The value of `source` run in the page, as the call's report shows it.
fn script(desk: &Desk, call: &Call, tab: &str, source: &str) -> Result<String, String> {
    let answer = window(
        desk,
        call,
        &json!({"action": "interact", "tab": tab, "script": source}),
    )?;
    let (_, after) = answer
        .split_once("### Script result\n")
        .ok_or_else(|| format!("no script result in: {answer}"))?;
    Ok(after.lines().next().unwrap_or_default().trim().to_string())
}

/// Whether the task's check holds now.
fn holds(desk: &Desk, call: &Call, tab: &str, task: &Task) -> Result<bool, String> {
    match (&task.check, &task.screen) {
        (Some(check), _) => Ok(script(desk, call, tab, check)? == "true"),
        (None, Some(text)) => {
            let screen = window(desk, call, &json!({"action": "interact", "tab": tab}))?;
            Ok(screen.contains(text.as_str()))
        }
        (None, None) => Err("a task has a check or a screen".into()),
    }
}

/// The helper's report read into `run`: how it ended, its steps, what the model cost.
fn helper(answer: &str, run: &mut Run) {
    let mut steps = 0;
    for line in answer.lines() {
        if let Some(outcome) = line.split_once("### Helper: ").map(|(_, o)| o) {
            run.claimed_done = outcome.starts_with("done");
            run.outcomes.push(outcome.to_string());
        } else if line
            .split_once(". ")
            .is_some_and(|(n, _)| n.parse::<usize>().is_ok())
        {
            steps += 1;
            run.trace.push(line.to_string());
        } else if let Some(usage) = line.strip_prefix("Model: ") {
            usage_into(usage, run);
        }
    }
    run.steps += steps;
}

/// "4 decisions, $0.0012, 2.3 s."
fn usage_into(usage: &str, run: &mut Run) {
    let parts: Vec<&str> = usage.trim_end_matches('.').split(", ").collect();
    if let [decisions, cost, time] = parts.as_slice() {
        run.decisions += decisions
            .split(' ')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        run.cost += cost.trim_start_matches('$').parse().unwrap_or(0.0);
        run.model_s += time.trim_end_matches(" s").parse().unwrap_or(0.0);
    }
}

/// The tasks in `path`.
pub(crate) fn tasks(path: &Path) -> Result<Vec<Task>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
