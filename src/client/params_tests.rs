//! Tests of `params`.

use super::*;

#[test]
fn a_call_carries_an_idempotency_key_and_its_environment_only_when_it_starts_a_program() {
    let listing = window_params("o", "/p", &json!({"action": "list"}));
    assert!(listing["env"].is_null() && listing["key"].is_null());
    assert_eq!(listing["idempotencyKey"].as_str().unwrap().len(), 32);
    let other = window_params("o", "/p", &json!({"action": "list"}));
    assert_ne!(listing["idempotencyKey"], other["idempotencyKey"]);
    let opening = window_params(
        "o",
        "/p",
        &json!({"action": "open", "kind": "terminal", "command": "sh"}),
    );
    assert!(opening["env"].is_object());
}
