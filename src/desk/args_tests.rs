//! Tests of argument validation against the window tool's schema.

use super::check;
use serde_json::json;

#[test]
fn a_right_call_passes() {
    let args = json!({"action": "interact", "tab": "w1", "read": true, "keys": ["Enter"], "yield_ms": 500});
    assert_eq!(check(&args), Ok(()));
}

#[test]
fn a_wrong_type_is_named_not_ignored() {
    let why = check(&json!({"action": "interact", "tab": "w1", "read": "h1"})).unwrap_err();
    assert!(
        why.starts_with("`read` takes a boolean, not \"h1\"."),
        "{why}"
    );
}

#[test]
fn an_array_of_the_wrong_items_is_refused() {
    let why = check(&json!({"action": "interact", "tab": "w1", "keys": "Enter"})).unwrap_err();
    assert!(why.starts_with("`keys` takes an array of strings"), "{why}");
    let why = check(&json!({"action": "interact", "tab": "w1", "keys": [13]})).unwrap_err();
    assert!(why.starts_with("`keys` takes an array of strings"), "{why}");
}

#[test]
fn a_null_field_is_left_alone_and_an_unknown_one_refused() {
    assert_eq!(
        check(&json!({"action": "list", "tab": null, "extra": null})),
        Ok(())
    );
    let why = check(&json!({"action": "open", "url": "x", "goal": "log in"})).unwrap_err();
    assert!(
        why.starts_with("no field `goal`: nothing was done"),
        "{why}"
    );
}

#[test]
fn the_schema_lists_every_op_a_board_takes() {
    let schema = super::schema();
    let ops: Vec<&str> = schema["properties"]["op"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|op| op.as_str())
        .collect();
    for op in ["look", "show", "card", "add", "remove"] {
        assert!(ops.contains(&op), "{op} is not in {ops:?}");
    }
}
