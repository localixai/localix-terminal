//! `window --watch`: what a plugin's line above the prompt shows, kept true by the app's events
//! alone. Each change is one JSON object on a line of its own:
//!
//! - `{"app": "llxterm 0.2.0 (pid 7)", "window": "open", "terminals": "2", "tabs": ["t1 …"]}`
//!   as the app is reached, and again when one of the caller's tabs opens, changes or closes
//!   (only when what it says changed);
//! - `{"app": "not running"}` when there is none, or it went;
//! - `{"board": {…}}` when the project's board changed, as the app keeps it (`null`: none);
//! - `{"said": "…"}` for each comment the person sends, acknowledged once printed.

use serde_json::{Value, json};

use super::{Step, printed, run, said};
use crate::client::{Caller, Client, window_params};

/// What the watcher subscribes to.
const EVENTS: [&str; 5] = [
    "comment/added",
    "tab/changed",
    "tab/closed",
    "board/changed",
    "app/quitting",
];

/// Runs until the program that started it ends; its exit code, 0.
pub fn watch(caller: &Caller) -> u8 {
    let mut last = String::new();
    run(caller, &EVENTS, |step| match step {
        Step::Connected(client) | Step::Note(client, "tab/changed" | "tab/closed", _) => {
            once(&mut last, &desk(client, caller));
        }
        Step::Note(_, "board/changed", told) => {
            printed(&json!({"board": told["board"]}).to_string());
        }
        Step::Note(client, "comment/added", note) => {
            printed(&json!({"said": said(client, caller, note)}).to_string());
        }
        Step::Note(_, "app/quitting", _) | Step::Away => {
            once(&mut last, &json!({"app": "not running"}));
        }
        Step::Note(..) => {}
    })
}

/// `line` printed unless it is what was printed `last`.
fn once(last: &mut String, line: &Value) {
    let line = line.to_string();
    if *last != line {
        printed(&line);
        *last = line;
    }
}

/// The app as `llxterm status` says it, and the caller's tabs as a look for a pane lists them
/// (`peek`: what the person sent stays for the agent's own call).
fn desk(client: &Client, caller: &Caller) -> Value {
    let status = client.request("app/status", json!({}));
    let text = (status.ok())
        .and_then(|answer| answer.value["text"].as_str().map(String::from))
        .unwrap_or_default();
    let field = |name: &str| {
        (text.lines())
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
            .unwrap_or("-")
            .to_string()
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut asked = window_params(
        caller,
        &cwd.display().to_string(),
        &json!({"action": "list"}),
    );
    asked["peek"] = json!(true);
    let listed = (client.request("window/call", asked).ok())
        .and_then(|answer| answer.value["text"].as_str().map(String::from))
        .unwrap_or_default();
    json!({
        "app": text.lines().next().unwrap_or("running"),
        "window": field("window"),
        "terminals": field("terminals"),
        "tabs": tabs(&listed),
    })
}

/// The lines of a list that name a tab: its key (`w1`, `t2`, `b1`) and a space first.
fn tabs(listed: &str) -> Vec<&str> {
    (listed.lines())
        .filter(|line| {
            line.split_once(' ').is_some_and(|(key, _)| {
                let mut chars = key.chars();
                let kind = matches!(chars.next(), Some('w' | 't' | 'b'));
                let number = chars.as_str();
                kind && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
