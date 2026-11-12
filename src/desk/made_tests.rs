//! Tests of `made`: a kind that draws itself, as an agent calls it over a desk with no window.

use serde_json::json;

use super::super::tests::{caller, desk};
use crate::kind::testing::Echo;
use crate::testing::Scratch;

#[test]
fn a_kind_the_desk_never_named_is_opened_told_read_listed_and_closed() {
    let echo = Echo::installed();
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let opened = desk
        .window(&me, &json!({"action": "open", "kind": "echo"}))
        .unwrap();
    assert!(opened.contains("x1 (echo): 0 heard"), "{opened}");
    let told = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "x1", "say": "hi"}),
        )
        .unwrap();
    assert!(told.starts_with("x1: said hi"), "{told}");
    assert_eq!(echo.heard(), ["call hi"]);
    let read = desk
        .window(&me, &json!({"action": "interact", "tab": "x1"}))
        .unwrap();
    assert!(read.contains("echo: call hi"), "{read}");
    let listed = desk.window(&me, &json!({"action": "list"})).unwrap();
    assert!(listed.contains("x1 (echo): 1 heard"), "{listed}");
    let closed = desk
        .window(&me, &json!({"action": "close", "tab": "x1"}))
        .unwrap();
    assert_eq!(closed, "Closed x1.");
    let gone = desk.window(&me, &json!({"action": "list"})).unwrap();
    assert_eq!(gone, "No tabs open.");
}

#[test]
fn a_project_has_one_tab_of_a_kind_and_a_closed_one_opens_under_a_new_key() {
    let _echo = Echo::installed();
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let open = json!({"action": "open", "kind": "echo"});
    assert!(desk.window(&me, &open).unwrap().contains("x1 (echo)"));
    assert!(desk.window(&me, &open).unwrap().contains("x1 (echo)"));
    desk.window(&me, &json!({"action": "close", "tab": "x1"}))
        .unwrap();
    assert!(desk.window(&me, &open).unwrap().contains("x2 (echo)"));
}

#[test]
fn a_field_of_another_kind_is_refused_on_its_tab_with_how_it_is_acted_on() {
    let _echo = Echo::installed();
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "echo"}))
        .unwrap();
    let refused = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "x1", "click": "e1"}),
        )
        .unwrap_err();
    let why = refused.message();
    assert!(why.contains("a echo tab takes no `click`"), "{why}");
    assert!(why.contains("An echo is told with `say`."), "{why}");
}

#[test]
fn what_the_kind_refuses_is_the_calls_failure() {
    let _echo = Echo::installed();
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "echo"}))
        .unwrap();
    let refused = desk
        .window(&me, &json!({"action": "interact", "tab": "x1", "say": 4}))
        .unwrap_err();
    assert!(
        refused.message().contains("`say` takes a string"),
        "{refused:?}"
    );
}

#[test]
fn the_tool_says_the_kind_in_its_schema_and_its_guide_and_none_without_it() {
    let bare = super::super::schema().to_string();
    assert!(!bare.contains("echo"), "{bare}");
    let _echo = Echo::installed();
    let schema = super::super::schema();
    let kinds = schema["properties"]["kind"]["enum"].to_string();
    assert!(kinds.contains("\"echo\""), "{kinds}");
    assert_eq!(schema["properties"]["say"]["type"], "string");
    let guide = super::super::description();
    assert!(
        guide.contains("- echo (`x` tab): Says back what it is told with `say`."),
        "{guide}"
    );
}

#[test]
fn another_project_does_not_reach_the_tab() {
    let _echo = Echo::installed();
    let (desk, one, two) = (desk(), Scratch::new(), Scratch::new());
    let (mine, theirs) = (
        caller(&desk, one.path(), None),
        caller(&desk, two.path(), None),
    );
    desk.window(&mine.call(), &json!({"action": "open", "kind": "echo"}))
        .unwrap();
    let refused = desk
        .window(
            &theirs.call(),
            &json!({"action": "interact", "tab": "x1", "say": "hi"}),
        )
        .unwrap_err();
    assert!(refused.message().contains("there is no tab x1 among yours"));
}
