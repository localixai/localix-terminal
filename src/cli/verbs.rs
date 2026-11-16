//! The short commands (`llxterm click e12 + wait Saved`): the words after `llxterm` made into the
//! steps of one `window` call by the one table (`crate::short`), and sent as `window` sends its
//! JSON. A step that misses fails the command (exit 1), so `&&` stops there.

use std::path::PathBuf;

use serde_json::json;

use super::OK;
use super::call::{Missed, printed_as, read_stdin, usage};
use crate::client::{Caller, Start, ask, window_params};
use crate::out::say;

/// The caller's own options among the words, before a `--`, and the words left.
#[derive(Default)]
struct Own {
    /// `--as NAME`: the agent's name within the project.
    agent: Option<String>,
    /// `--project DIR`: the project, when the caller knows it better than its directory does.
    project: Option<PathBuf>,
    rest: Vec<String>,
}

fn own(argv: &[String]) -> Result<Own, String> {
    let mut parsed = Own::default();
    let mut words = argv.iter();
    while let Some(word) = words.next() {
        let value = |next: Option<&String>, what: &str| {
            (next.filter(|value| !value.starts_with("--")).cloned())
                .ok_or_else(|| format!("{word} needs {what}"))
        };
        match word.as_str() {
            "--as" => parsed.agent = Some(value(words.next(), "a name")?),
            "--project" => parsed.project = Some(PathBuf::from(value(words.next(), "a folder")?)),
            // The rest are a step's values, as written.
            "--" => {
                parsed.rest.push(word.clone());
                parsed.rest.extend(words.by_ref().cloned());
            }
            _ => parsed.rest.push(word.clone()),
        }
    }
    Ok(parsed)
}

/// `argv`, a verb first, run as one call of its steps.
pub(super) fn run(argv: &[String]) -> u8 {
    let options = argv.iter().take_while(|word| *word != "--");
    if options
        .into_iter()
        .any(|word| word == "-h" || word == "--help")
    {
        say(&super::help());
        return OK;
    }
    let given = match own(argv) {
        Ok(given) => given,
        Err(why) => return usage(&why),
    };
    let steps = match crate::short::steps(&given.rest, &mut read_stdin) {
        Ok(steps) => steps,
        Err(why) => return usage(&why),
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let caller = match &given.project {
        Some(dir) => Caller::at(&cwd.join(dir), given.agent),
        None => Caller::of(&cwd, given.agent),
    };
    let args = json!({"action": "steps", "steps": steps});
    let params = window_params(&caller, &cwd.display().to_string(), &args);
    printed_as(ask(Start::Spawn, "window/call", &params), Missed::Failed)
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
