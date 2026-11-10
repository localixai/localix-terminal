//! Tests of `board_run`.

use serde_json::json;

use super::*;

fn started(agent: &str, prompt: &str) -> Result<String, String> {
    match agent {
        "codex" => Ok(format!("codex '{prompt}'")),
        other => Err(format!("{other} is not installed")),
    }
}

#[test]
fn a_run_names_the_agents_worker_and_starts_it_on_its_card() {
    let (action, run) = prepared(
        json!({"op": "run", "card": "C3", "agent": "codex"}),
        "b1",
        started,
    )
    .unwrap();
    assert_eq!(action["who"], "codex-c3");
    let run = run.unwrap();
    assert_eq!(run.title, "c3 · codex");
    assert!(run.command.starts_with("codex '"), "{}", run.command);
    for told in [
        "card c3",
        "tab b1",
        "is codex-c3",
        "\"op\": \"review\"",
        "\"who\":\"codex-c3\"",
    ] {
        assert!(run.command.contains(told), "{told}: {}", run.command);
    }
}

#[test]
fn another_action_is_the_boards_and_a_run_without_its_agent_is_refused() {
    let accept = json!({"op": "accept", "card": "c1"});
    assert_eq!(
        prepared(accept.clone(), "b1", started).unwrap(),
        (accept, None)
    );
    let missing = prepared(
        json!({"op": "run", "card": "c1", "agent": "gemini"}),
        "b1",
        started,
    );
    assert_eq!(missing.unwrap_err(), "gemini is not installed");
    assert!(prepared(json!({"op": "run", "card": "c1"}), "b1", started).is_err());
}
