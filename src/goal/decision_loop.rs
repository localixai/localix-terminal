//! The decision loop: the hands that carry a task out on a view with a decision model alone. A
//! goal run stops on its own guards (the same action twice without a change, a view seen again, a
//! refused action, its share of actions); the loop resumes it with everything done so far as its
//! history, until the model says DONE or BLOCKED, the action budget is spent, or runs keep
//! stopping without changing the view.

use std::time::Duration;

use crate::hands::{Hands, Outcome, Report, Task};
use crate::model::Decider;
use crate::view::View;

use crate::goal::run::{GoalOutcome, GoalRun, Stop};
use crate::goal::step::{GoalStep, Operation};

/// Actions one goal run takes before the loop looks at how it stopped.
const SEGMENT: usize = 20;
/// Runs in a row that stopped without a step that changed the view: the task is stuck.
const MAX_STALLS: usize = 2;

pub struct DecisionLoop<'a> {
    pub decider: &'a dyn Decider,
    pub on_step: &'a (dyn Fn(&GoalStep) + Sync),
    /// Waits before a view is looked at again ([`GoalRun::pause`]).
    pub pause: &'a (dyn Fn(Duration) + Sync),
    /// Actions for one task.
    pub max_actions: usize,
}

fn actions(steps: &[GoalStep]) -> usize {
    steps
        .iter()
        .filter(|step| !matches!(step.operation, Operation::Wait | Operation::Done))
        .count()
}

impl DecisionLoop<'_> {
    /// Drive `view` toward `goal`, typing only `values`.
    pub fn run(&self, view: &dyn View, goal: &str, values: &[String]) -> GoalOutcome {
        let mut steps = vec![];
        let mut withheld: Vec<String> = vec![];
        let (mut decisions, mut cost, mut millis, mut stalls) = (0, 0.0, 0, 0);
        loop {
            let left = self.max_actions.saturating_sub(actions(&steps));
            let before = steps.len();
            let mut outcome = GoalRun {
                view,
                decider: self.decider,
                on_step: self.on_step,
                pause: self.pause,
                max_actions: SEGMENT.min(left),
            }
            .resume(goal, values, steps);
            decisions += outcome.decisions;
            cost += outcome.cost;
            millis += outcome.decision_millis;
            for why in std::mem::take(&mut outcome.withheld) {
                if !withheld.contains(&why) {
                    withheld.push(why);
                }
            }
            let moved = outcome.steps[before..]
                .iter()
                .any(|step| step.changed == Some(true) && step.failed.is_none());
            let again = match &outcome.stop {
                Stop::Done { .. } | Stop::Blocked | Stop::Unreachable(_) => false,
                // Its share of actions spent: a checkpoint, not a failure.
                Stop::Limit => left > SEGMENT,
                // Going round in circles changes the view at every step without getting anywhere:
                // a stall however much moved.
                // So is the same action again with nothing new to read: what it stirred (a form
                // setting the same error) is no progress.
                Stop::Cycling | Stop::Repeated => {
                    stalls += 1;
                    stalls < MAX_STALLS && left > 0
                }
                Stop::ActionFailed(_) | Stop::NoDecision(_) => {
                    stalls = if moved { 0 } else { stalls + 1 };
                    stalls < MAX_STALLS && left > 0
                }
            };
            if !again {
                outcome.decisions = decisions;
                outcome.cost = cost;
                outcome.decision_millis = millis;
                outcome.withheld = withheld;
                return outcome;
            }
            steps = outcome.steps;
        }
    }
}

impl Hands for DecisionLoop<'_> {
    fn carry_out(&self, view: &dyn View, task: Task) -> Report {
        let run = self.run(view, task.goal, task.values);
        let outcome = match &run.stop {
            Stop::Done { probability } => Outcome::Done {
                probability: *probability,
            },
            Stop::Blocked => Outcome::Blocked,
            Stop::Limit => Outcome::Exhausted,
            Stop::Unreachable(error) => Outcome::Unreachable(error.clone()),
            other @ (Stop::Repeated
            | Stop::Cycling
            | Stop::ActionFailed(_)
            | Stop::NoDecision(_)) => Outcome::Stopped(other.to_string()),
        };
        // One line per control, its steps together: "steps 2, 3 and 5, button "Save": …".
        let mut by_control: Vec<(String, crate::goal::commit::Commit, Vec<usize>)> = vec![];
        for (i, step) in run.steps.iter().enumerate() {
            let Some(commit) = step.commit.filter(|_| step.failed.is_none()) else {
                continue;
            };
            let control = step.control.clone().unwrap_or_else(|| "a control".into());
            match by_control
                .iter_mut()
                .find(|(c, k, _)| *c == control && *k == commit)
            {
                Some((_, _, steps)) => steps.push(i + 1),
                None => by_control.push((control, commit, vec![i + 1])),
            }
        }
        let commits = (by_control.into_iter())
            .map(|(control, commit, steps)| {
                format!("{}, {control}: {}", steps_named(&steps), commit.done())
            })
            .collect();
        let paid = (run.steps.iter())
            .filter(|step| {
                step.commit == Some(crate::goal::commit::Commit::Pays)
                    && step.failed.is_none()
                    && step.changed == Some(true)
            })
            .filter_map(|step| step.control.clone())
            .collect();
        Report {
            decisions: run.decisions,
            cost: run.cost,
            model_millis: run.decision_millis,
            paid,
            steps: run.steps.iter().map(GoalStep::trace_line).collect(),
            outcome,
            view: run.view,
            commits,
            withheld: run.withheld,
        }
    }
}

/// "step 2", or "steps 2, 3 and 5".
fn steps_named(steps: &[usize]) -> String {
    match steps {
        [one] => format!("step {one}"),
        [rest @ .., last] => {
            let rest: Vec<String> = rest.iter().map(ToString::to_string).collect();
            format!("steps {} and {last}", rest.join(", "))
        }
        [] => String::new(),
    }
}

#[cfg(test)]
#[path = "decision_loop_tests.rs"]
mod tests;
