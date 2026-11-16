//! Tests of what a terminal of the app's is told about itself, and of the terminal a caller's
//! environment names.

use serde_json::json;

use super::super::super::tests::{caller, desk};
use crate::testing::Scratch;

#[test]
fn a_terminal_says_which_terminal_and_project_it_is_and_that_it_reads_program_status() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "terminal",
        "command": r#"printf 'is=%s:%s:%s.' "$LLXTERM_PANE" "$LLXTERM_PANE_PROJECT" "$LLXTERM_PROGRAM_STATUS"; sleep 20"#,
        "wait_for": "."});
    let text = desk.window(&mine.call(), &open).unwrap();
    let folder = mine.project.path.display();
    assert!(text.contains(&format!("is=t1:{folder}:1.")), "{text}");
    desk.shutdown();
}

#[test]
fn a_call_names_its_terminal_by_the_variable_while_that_terminal_is_open() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 200});
    desk.window(&mine.call(), &open).unwrap();
    let named = |key: &str| {
        let vars = [("LLXTERM_PANE".to_string(), key.to_string())];
        desk.pane_named(Some(&vars))
    };
    assert_eq!(named("t1").as_deref(), Some("t1"));
    // A terminal that is not open, or a word that is no terminal's key, names none.
    assert_eq!(named("t7"), None);
    assert_eq!(named("w1"), None);
    assert_eq!(named("t1; rm"), None);
    assert_eq!(desk.pane_named(None), None);
    assert_eq!(desk.pane_named(Some(&[])), None);
    desk.shutdown();
}
