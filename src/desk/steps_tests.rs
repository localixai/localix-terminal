//! Tests of several steps in one call, on terminals (no window): every step is checked before
//! the first runs, they run in order on the tab the one before used, the first that fails or
//! misses stops the rest, and the answer is a line a step and the last step's tab once.

use serde_json::{Value, json};

use super::super::tests::{caller, desk};
use crate::testing::Scratch;

/// A program that says it is ready, then answers each line it reads.
const ECHO: &str =
    r#"term "sh -c 'printf \"ready> \"; while read l; do echo got:$l; done'" --wait "ready>""#;

fn steps(given: &[Value]) -> Value {
    json!({"action": "steps", "steps": given})
}

#[test]
fn steps_run_in_order_on_the_tab_before_and_the_screen_is_read_once() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let call = steps(&[json!(ECHO), json!("type hi --enter"), json!("wait got:hi")]);
    let answered = desk.call(&me, &call).unwrap();
    let text = &answered.text;
    assert!(!answered.missed, "{text}");
    assert!(
        text.starts_with("1. t1 (terminal): \"ready>\" showed.")
            && text.contains("\n2. t1: ")
            && text.contains("\n3. t1: \"got:hi\" showed."),
        "{text}"
    );
    // The middle steps are a line each: the screen is the last step's alone.
    assert_eq!(
        text.matches("(untrusted program output)").count(),
        1,
        "{text}"
    );
    // One step is answered as the call it is, with no number.
    let read = desk.call(&me, &steps(&[json!("read")])).unwrap();
    assert!(read.text.starts_with("t1: "), "{}", read.text);
    desk.shutdown();
}

#[test]
fn the_first_step_that_misses_stops_the_rest() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let call = steps(&[
        json!(ECHO),
        json!("wait never-shown --ms 300"),
        json!("type later --enter"),
    ]);
    let answered = desk.call(&me, &call).unwrap();
    let text = &answered.text;
    assert!(answered.missed, "{text}");
    assert!(
        text.contains("2. t1: \"never-shown\" did not show")
            && text.contains("Stopped at step 2 of 3: step 3 was not done."),
        "{text}"
    );
    // Step 3 never typed.
    let screen = desk.window(&me, &json!({"action": "interact", "tab": "t1"}));
    assert!(!screen.unwrap().contains("got:later"));

    // A single call that waited for text that never came missed too; it is answered as before.
    let alone = json!({"action": "interact", "tab": "t1", "wait_for": "nope", "yield_ms": 200});
    let answered = desk.call(&me, &alone).unwrap();
    assert!(answered.missed && answered.text.contains("did not show"));
    desk.shutdown();
}

#[test]
fn every_step_is_checked_before_any_runs() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let refused = |given: Value| desk.call(&me, &given).err().unwrap().message().to_string();

    let why = refused(steps(&[
        json!(ECHO),
        json!({"action": "open", "kind": "terminal", "bogus": 1}),
    ]));
    assert!(
        why.starts_with("step 2: no field `bogus`") && why.contains("nothing was done"),
        "{why}"
    );
    let why = refused(steps(&[json!(ECHO), json!("clik e1")]));
    assert!(why.starts_with("step 2: no command `clik`"), "{why}");
    let why = refused(steps(&[json!(ECHO), json!("shot"), json!("list")]));
    assert!(why.contains("step 2") && why.contains("last"), "{why}");
    let why = refused(steps(&[json!({"action": "steps", "steps": ["list"]})]));
    assert!(why.contains("step 1"), "{why}");
    let why = refused(steps(&vec![json!("list"); 17]));
    assert!(why.contains("16"), "{why}");
    let why = refused(steps(&[]));
    assert!(why.contains("nothing was done"), "{why}");
    let why = refused(steps(&[json!(7)]));
    assert!(why.starts_with("step 1: "), "{why}");
    // None of them ran: no terminal was opened.
    let listed = desk.window(&me, &json!({"action": "list"})).unwrap();
    assert_eq!(listed, "No tabs open.");
    desk.shutdown();
}

#[test]
fn a_step_with_no_tab_and_none_of_its_kind_says_how_to_open_one() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let why = desk.call(&me, &steps(&[json!("type hi")])).err().unwrap();
    assert!(
        why.message().contains("no terminal open") && why.message().contains("llxterm term"),
        "{}",
        why.message()
    );
    let why = desk.call(&me, &steps(&[json!("click e1")])).err().unwrap();
    assert!(why.message().contains("no page open"), "{}", why.message());
    // A step that fails after another was done says what was done, and stops.
    let call = steps(&[json!(ECHO), json!("click e1"), json!("list")]);
    let why = desk.call(&me, &call).err().unwrap();
    let why = why.message();
    assert!(
        why.starts_with("1. t1 (terminal): ")
            && why.contains("\n2. ")
            && why.contains("Stopped at step 2 of 3"),
        "{why}"
    );
    desk.shutdown();
}

#[test]
fn the_guide_is_an_action_of_the_tool_itself() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let commands = desk
        .window(&me, &json!({"action": "guide", "topic": "commands"}))
        .unwrap();
    assert!(commands.contains("llxterm click [TAB]"), "{commands}");
    let overview = desk.window(&me, &json!({"action": "guide"})).unwrap();
    assert!(overview.contains("- pages: "), "{overview}");
    let why = desk
        .window(&me, &json!({"action": "guide", "topic": "more"}))
        .unwrap_err();
    assert!(
        why.message().starts_with("no topic `more`"),
        "{}",
        why.message()
    );
    desk.shutdown();
}
