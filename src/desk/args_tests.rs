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
fn null_and_unknown_fields_are_left_alone() {
    assert_eq!(
        check(&json!({"action": "list", "tab": null, "extra": 1})),
        Ok(())
    );
}
