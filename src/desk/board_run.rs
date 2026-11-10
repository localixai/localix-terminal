//! A card run by a coding agent the person picks on the board: the card goes to Doing as the
//! agent's own (a worker named `codex-c3`), and the agent starts in a terminal of the person's,
//! in the project's folder, on a first prompt that says which card is its and how the board is
//! worked. The desk knows nothing more of the agent: it works the card as any worker does.

use serde_json::Value;

/// A terminal to start once the board has taken the run: its command line and its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Started {
    pub(super) command: String,
    pub(super) title: String,
}

/// The person's `action` on board tab `key`, made ready for the board: a `run` names its worker
/// (`who`) and gives the terminal to start; any other action is the board's as it is.
pub(super) fn prepared(
    mut action: Value,
    key: &str,
    start: impl Fn(&str, &str) -> Result<String, String>,
) -> Result<(Value, Option<Started>), String> {
    if action.get("op").and_then(Value::as_str) != Some("run") {
        return Ok((action, None));
    }
    let word = |name: &str| {
        (action.get(name).and_then(Value::as_str))
            .map(|text| text.trim().to_ascii_lowercase())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("`{name}` is needed"))
    };
    let (agent, card) = (word("agent")?, word("card")?);
    let who = format!("{agent}-{card}");
    let command = start(&agent, &prompt(&card, &who, key))?;
    action["who"] = Value::String(who);
    let title = format!("{card} · {}", command.split(' ').next().unwrap_or(&agent));
    Ok((action, Some(Started { command, title })))
}

/// What the agent is told first: its card, its name on the board, and how to work it there.
pub(super) fn prompt(card: &str, who: &str, key: &str) -> String {
    let read = serde_json::json!({"action": "interact", "tab": key, "op": "card", "card": card, "who": who});
    format!(
        "Work card {card} of this project's llx-desk board (its tab {key}). Your name on the board is \
         {who}, and the card is already yours in Doing. Read it with `llx-desk window '{read}'`: its \
         title and note say what to do and how to tell it is done. Pass \"who\": \"{who}\" in every \
         call on the board. Keep to what the card asks. When the work needs the person, `block` it \
         with a `reason`. When it is done and checked, hand it in for the person's review \
         (\"op\": \"review\", with a `note` of what you did and how you checked it); never move it \
         to Done yourself."
    )
}

#[cfg(test)]
#[path = "board_run_tests.rs"]
mod tests;
