//! Tests of `desk`.

use serde_json::json;

use super::*;
use crate::testing::Scratch;

fn call<'a>(owner: &'a str, cwd: &'a Path) -> Call<'a> {
    Call {
        owner,
        cwd,
        vars: None,
        key: None,
        progress: None,
    }
}

fn desk() -> Arc<Desk> {
    Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        Arc::new(Quiet),
    )
}

#[test]
fn a_terminal_is_opened_typed_into_listed_and_ended_by_its_owner() {
    let (desk, scratch) = (desk(), Scratch::new());
    let me = call("me", scratch.path());
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'printf \"ready> \"; read x; echo got-$x; sleep 20'", "wait_for": "ready>"});
    let text = desk.window(&me, &open).unwrap();
    assert!(
        text.contains("t1 (terminal)") && text.contains("ready>"),
        "{text}"
    );

    let typed = json!({"action": "interact", "tab": "t1", "input": "7\n", "wait_for": "got-7"});
    assert!(desk.window(&me, &typed).unwrap().contains("got-7"));
    assert!(
        desk.window(&me, &json!({"action": "list"}))
            .unwrap()
            .contains("t1 (terminal)")
    );
    assert_eq!(desk.end("me"), 1);
    assert_eq!(
        desk.window(&me, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
}

#[test]
fn another_owner_neither_sees_nor_reaches_a_tab() {
    let (desk, scratch) = (desk(), Scratch::new());
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&call("a", scratch.path()), &open).unwrap();
    let other = call("b", scratch.path());
    assert_eq!(
        desk.window(&other, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
    let refused = desk
        .window(&other, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(refused.contains("among yours"), "{refused}");
    desk.shutdown();
}

#[test]
fn a_page_needs_a_window_and_a_bad_action_says_what_is_known() {
    let (desk, scratch) = (desk(), Scratch::new());
    let me = call("me", scratch.path());
    let no_window = desk.window(
        &me,
        &json!({"action": "open", "kind": "browser", "url": "https://example.com"}),
    );
    assert!(no_window.unwrap().contains("no window here"));
    let Err(Failure::Failed(why)) = desk.window(&me, &json!({"action": "dance"})) else {
        panic!("an unknown action was taken");
    };
    assert!(why.contains("open, interact"), "{why}");
}

#[test]
fn what_the_person_sent_reaches_the_owner_of_the_tab_once() {
    let (desk, scratch) = (desk(), Scratch::new());
    let me = call("me", scratch.path());
    desk.window(
        &me,
        &json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300}),
    )
    .unwrap();
    crate::lock(&desk.registry).hear("t1", "this button is wrong");
    let said = desk.said("me").unwrap();
    assert!(
        said.contains("this button is wrong") && said.contains("untrusted"),
        "{said}"
    );
    assert!(desk.said("me").is_none());
    desk.shutdown();
}

/// Records what the desk says happened.
#[derive(Default)]
struct Told(Mutex<Vec<(String, Option<String>, Value)>>);

impl Events for Told {
    fn publish(&self, event: &str, owner: Option<&str>, params: Value) {
        crate::lock(&self.0).push((event.to_string(), owner.map(String::from), params));
    }
}

#[test]
fn a_comment_is_announced_kept_until_acknowledged_and_a_closed_tab_is_announced() {
    let told = Arc::new(Told::default());
    let events: Arc<dyn Events> = Arc::<Told>::clone(&told);
    let desk = Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        events,
    );
    let scratch = Scratch::new();
    let me = call("me", scratch.path());
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&me, &open).unwrap();

    desk.person_says("t1", "this button is wrong");
    let said = desk.pending("me");
    assert_eq!(said.len(), 1);
    {
        let told = crate::lock(&told.0);
        assert_eq!(told[0].0, "comment/added");
        assert_eq!(told[0].1.as_deref(), Some("me"));
        assert_eq!(told[0].2["text"], "this button is wrong");
        assert_eq!(told[0].2["id"], said[0].id);
    }
    // Told is not delivered: it stays until a call carries it, or it is acknowledged.
    assert_eq!(desk.pending("me").len(), 1);
    desk.acknowledge("me", &[said[0].id]);
    assert!(desk.pending("me").is_empty());
    assert!(desk.said("me").is_none());

    desk.window(&me, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    let told = crate::lock(&told.0);
    assert!(
        told.iter()
            .any(|(event, owner, params)| event == "tab/closed"
                && owner.as_deref() == Some("me")
                && params["tab"] == "t1")
    );
    drop(told);
    desk.shutdown();
}

#[test]
fn waiting_for_text_the_input_echoes_waits_for_the_output_not_the_echo() {
    let (desk, scratch) = (desk(), Scratch::new());
    let me = call("me", scratch.path());
    let open = json!({"action": "open", "kind": "terminal", "command": "sh", "yield_ms": 500});
    desk.window(&me, &open).unwrap();
    let began = std::time::Instant::now();
    let typed = json!({"action": "interact", "tab": "t1",
        "input": "sleep 1; echo zz-done\n", "wait_for": "zz-done"});
    let text = desk.window(&me, &typed).unwrap();
    assert!(
        began.elapsed() >= std::time::Duration::from_millis(900),
        "it returned on the echo: {text}"
    );
    assert!(text.contains("\"zz-done\" showed"), "{text}");
    assert_eq!(text.matches("zz-done").count(), 3, "{text}");
    assert_eq!(desk.end("me"), 1);
}
