//! A benchmark's runs written down: each run a line of `bench/runs/<when>_<build>.jsonl`, the
//! whole a row of `bench/SCORES.md`, printed with the row before it to compare.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use super::run::Run;

/// The totals of one benchmark.
pub(crate) struct Score {
    pub(crate) passed: usize,
    pub(crate) runs: usize,
    pub(crate) safety_passed: usize,
    pub(crate) safety_runs: usize,
    /// Runs whose helper said done while the check did not hold.
    pub(crate) false_done: usize,
    pub(crate) steps: usize,
    pub(crate) decisions: usize,
    pub(crate) cost: f64,
    pub(crate) model_s: f64,
    pub(crate) wall_s: f64,
}

pub(crate) fn score(runs: &[Run]) -> Score {
    let counted: Vec<&Run> = runs
        .iter()
        .filter(|r| r.valid || r.error.is_some())
        .collect();
    let safety: Vec<&&Run> = counted.iter().filter(|r| r.safety).collect();
    Score {
        passed: counted.iter().filter(|r| r.passed).count(),
        runs: counted.len(),
        safety_passed: safety.iter().filter(|r| r.passed).count(),
        safety_runs: safety.len(),
        false_done: counted
            .iter()
            .filter(|r| r.claimed_done && !r.passed)
            .count(),
        steps: counted.iter().map(|r| r.steps).sum(),
        decisions: counted.iter().map(|r| r.decisions).sum(),
        cost: counted.iter().map(|r| r.cost).sum(),
        model_s: counted.iter().map(|r| r.model_s).sum(),
        wall_s: counted.iter().map(|r| r.wall_s).sum(),
    }
}

/// `git describe`-like: the commit, `+dirty` with changes not committed.
pub(crate) fn build(root: &Path) -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .unwrap_or_default()
    };
    let commit = git(&["rev-parse", "--short", "HEAD"]);
    let dirty = !git(&[
        "status",
        "--porcelain",
        "--",
        "src",
        "tests",
        "bench/tasks.json",
    ])
    .is_empty();
    format!("{commit}{}", if dirty { "+dirty" } else { "" })
}

/// Now, in UTC: `2026-11-01T18:30:00Z`.
pub(crate) fn now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

const HEAD: &str = "| when (UTC) | build | model | score | safety | false DONE | steps | requests | cost $ | model s | wall s | note |\n\
                    |---|---|---|---|---|---|---|---|---|---|---|---|\n";

fn row(when: &str, build: &str, model: &str, score: &Score, note: &str) -> String {
    format!(
        "| {when} | {build} | {model} | {}/{} | {}/{} | {} | {} | {} | {:.4} | {:.1} | {:.1} | {note} |\n",
        score.passed,
        score.runs,
        score.safety_passed,
        score.safety_runs,
        score.false_done,
        score.steps,
        score.decisions,
        score.cost,
        score.model_s,
        score.wall_s,
    )
}

/// Writes the runs and the score; returns the row before this one, to compare with.
pub(crate) fn write(
    dir: &Path,
    meta: (&str, &str, &str, &str),
    runs: &[Run],
    score: &Score,
) -> Result<Option<String>, String> {
    let (when, build, model, note) = meta;
    let runs_dir = dir.join("runs");
    std::fs::create_dir_all(&runs_dir).map_err(|e| e.to_string())?;
    let mut lines = String::new();
    for run in runs {
        let _ = writeln!(
            lines,
            "{}",
            serde_json::to_string(run).map_err(|e| e.to_string())?
        );
    }
    let file = runs_dir.join(format!("{}_{build}.jsonl", when.replace(':', "")));
    std::fs::write(&file, lines).map_err(|e| e.to_string())?;
    let scores = dir.join("SCORES.md");
    let before = std::fs::read_to_string(&scores).unwrap_or_default();
    let previous = before
        .lines()
        .rev()
        .find(|l| l.starts_with("| 2"))
        .map(String::from);
    let mut text = if before.contains("| when (UTC) |") {
        before
    } else {
        format!(
            "# Benchmark scores\n\nOne row per run of `bench/tasks.json`; each run's details are in `bench/runs/`.\n\n{HEAD}"
        )
    };
    text.push_str(&row(when, build, model, score, note));
    std::fs::write(&scores, text).map_err(|e| e.to_string())?;
    Ok(previous)
}

/// The per-task table and the totals, for the terminal.
pub(crate) fn table(runs: &[Run], score: &Score) -> String {
    let mut out = String::from("task                         passed  steps  requests  cost $\n");
    let mut ids: Vec<&str> = vec![];
    for run in runs {
        if !ids.contains(&run.task.as_str()) {
            ids.push(&run.task);
        }
    }
    for id in ids {
        let of: Vec<&Run> = runs.iter().filter(|r| r.task == id).collect();
        let n = of.len();
        let passed = of.iter().filter(|r| r.passed).count();
        let mark = if of.iter().any(|r| !r.valid && r.error.is_some()) {
            " (invalid)"
        } else {
            ""
        };
        let avg = |f: &dyn Fn(&Run) -> f64| of.iter().map(|r| f(r)).sum::<f64>() / n.max(1) as f64;
        let _ = writeln!(
            out,
            "{id:<28} {passed:>3}/{n:<3} {:>6.1} {:>9.1} {:>7.4}{mark}",
            avg(&|r| r.steps as f64),
            avg(&|r| r.decisions as f64),
            avg(&|r| r.cost),
        );
    }
    let _ = write!(
        out,
        "\nscore {}/{}, safety {}/{}, false DONE {}, {} steps, {} requests, ${:.4}, model {:.1} s, wall {:.1} s",
        score.passed,
        score.runs,
        score.safety_passed,
        score.safety_runs,
        score.false_done,
        score.steps,
        score.decisions,
        score.cost,
        score.model_s,
        score.wall_s
    );
    out
}
