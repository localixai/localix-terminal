//! Tests of `params`.

use super::*;

fn caller() -> Caller {
    Caller {
        project: PathBuf::from("/p"),
        agent: None,
    }
}

#[test]
fn a_call_carries_an_idempotency_key_and_its_environment_only_when_it_starts_a_program() {
    let listing = window_params(&caller(), "/p", &json!({"action": "list"}));
    assert!(listing["env"].is_null());
    assert_eq!(listing["idempotencyKey"].as_str().unwrap().len(), 32);
    let other = window_params(&caller(), "/p", &json!({"action": "list"}));
    assert_ne!(listing["idempotencyKey"], other["idempotencyKey"]);
    let opening = window_params(
        &caller(),
        "/p",
        &json!({"action": "open", "kind": "terminal", "command": "sh"}),
    );
    assert!(opening["env"].is_object());
    // A word in a value is no action.
    let filling = window_params(
        &caller(),
        "/p",
        &json!({"action": "interact", "fill": {"e1": "open"}}),
    );
    assert!(filling["env"].is_null());
}

#[test]
fn the_variables_an_integration_names_its_own_stay_out() {
    let vars = [
        "CLAUDECODE",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_CODE_USE_BEDROCK",
        "PATH",
        "LLX_DESK_SCRUB",
    ]
    .map(|name| (name.to_string(), "1".to_string()));
    let kept = scrubbed_of(vars.into_iter(), "CLAUDECODE, CLAUDE_PLUGIN_*,*");
    let names: Vec<_> = kept.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["CLAUDE_CODE_USE_BEDROCK", "PATH"]);
}
