//! Tests of `terminals`: what a terminal's program says of its state (OSC 7501) as the Event
//! Center shows it: what waits for the person and what failed want them, what is done is listed
//! quietly, and what is at work or at rest is no event.

use base64::Engine;
use serde_json::json;

use super::super::Kind;
use super::super::tests::shown;
use crate::desk::Call;
use crate::desk::tests::{caller, desk};
use crate::testing::Scratch;

fn b64(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

/// A terminal whose program writes the reports `bodies`, then `ready`, and waits.
fn reporting(bodies: &[String]) -> serde_json::Value {
    let reports = (bodies.iter())
        .map(|body| format!("\\033]7501;{body}\\007"))
        .collect::<Vec<_>>()
        .join("");
    let command = format!("printf '{reports}ready'; sleep 20");
    json!({"action": "open", "kind": "terminal", "command": command, "wait_for": "ready"})
}

#[test]
fn a_program_that_waits_for_the_person_is_one_line_on_its_terminal_named_by_its_app() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let asks = format!(
        "state=blocked:app=claude-code:kind=permission:msg={}",
        b64("approve Bash: touch /tmp/x")
    );
    let working = "state=working:id=a1:title=".to_string() + &b64("a background agent");
    desk.window(&me, &reporting(&[asks, working])).unwrap();
    let events = shown(&desk);
    assert_eq!(events.len(), 1, "what is at work is no event: {events:?}");
    let asks = &events[0];
    assert_eq!((asks.kind, asks.tab.as_str()), (Kind::Waiting, "t1"));
    // The title is the terminal's own, so a program never speaks as the app: the name it gives
    // itself is in the detail.
    assert!(asks.title.starts_with("printf "), "{}", asks.title);
    assert_eq!(asks.detail, "Claude Code · approve Bash: touch /tmp/x");
    assert_eq!(
        asks.words, "approve Bash: touch /tmp/x",
        "its tab waits with it"
    );
    assert_eq!(asks.labels(), ["Show", "Dismiss"]);
    desk.shutdown();
}

#[test]
fn what_a_program_waits_for_is_said_when_it_gives_no_words() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let bodies = [
        "state=blocked:app=codex:kind=question".to_string(),
        "state=blocked:id=a:kind=permission".into(),
        "state=blocked:id=b:kind=auth".into(),
        "state=blocked:id=c".into(),
    ];
    desk.window(&me, &reporting(&bodies)).unwrap();
    let mut said: Vec<String> = (shown(&desk).into_iter())
        .map(|event| event.detail)
        .collect();
    said.sort();
    assert_eq!(
        said,
        [
            "Codex · Has a question",
            "Codex · Needs approval",
            "Codex · Needs you to sign in",
            "Codex · Waits for you",
        ]
    );
    desk.shutdown();
}

#[test]
fn a_failure_wants_the_person_and_what_is_done_is_listed_quietly_until_they_type() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let bodies = [
        "state=done:app=claude-code".to_string(),
        format!("state=error:id=a1:title={}", b64("the cart's tests")),
        "state=done:id=a2".into(),
    ];
    desk.window(&me, &reporting(&bodies)).unwrap();
    let events = shown(&desk);
    let lines: Vec<_> = (events.iter())
        .map(|event| (event.kind, event.detail.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (Kind::Failed, "Claude Code · the cart's tests"),
            (Kind::Finished, "Claude Code · Done"),
        ],
        "a part that is done is the program's own business"
    );
    assert!(events[0].look.urgent && !events[1].look.urgent);
    desk.session(1).unwrap().typed("y");
    assert!(shown(&desk).is_empty(), "seen, both go");
    desk.shutdown();
}

#[test]
fn a_program_that_waits_again_after_the_person_typed_is_a_new_line() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let command = r"printf '\033]7501;state=blocked\007one'; read x; printf '\033]7501;state=working\007\033]7501;state=blocked\007two'; sleep 20";
    let open = json!({"action": "open", "kind": "terminal", "command": command, "wait_for": "one"});
    desk.window(&me, &open).unwrap();
    let first = shown(&desk);
    assert_eq!(first.len(), 1);
    // The person answers in the terminal; the program asks again.
    let session = desk.session(1).unwrap();
    session.typed("y\r");
    let text = crate::desk::restore::agent_resume::tests::soon(
        || session.text(),
        |text| text.contains("two"),
    );
    assert!(text.contains("two"), "{text}");
    let again = shown(&desk);
    assert_eq!(again.len(), 1);
    assert_ne!(first[0].id, again[0].id, "{:?}", again[0].id);
    desk.shutdown();
}

#[test]
fn a_program_flipping_its_state_is_one_line_and_floods_neither_the_feed_nor_the_journal() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let command = r"i=0; while [ $i -lt 150 ]; do printf '\033]7501;state=blocked:app=codex\007\033]7501;state=working:app=codex\007'; sleep 0.01; i=$((i+1)); done; printf '\033]7501;state=blocked:app=codex\007ready'; sleep 20";
    let open =
        json!({"action": "open", "kind": "terminal", "command": command, "wait_for": "ready"});
    desk.window(&me, &open).unwrap();
    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let count = |sql: &str| -> u64 {
        desk.feed
            .store()
            .read(|db| {
                db.query_row(sql, [], |row| row.get(0))
                    .map_err(|why| why.to_string())
            })
            .unwrap()
    };
    let notices = count("SELECT COUNT(*) FROM change_events WHERE entity LIKE '%erminal%'");
    assert!(notices < 20, "{notices} notices of one terminal");
    let rows = count("SELECT COUNT(*) FROM center_events WHERE id LIKE 'status:%'");
    assert_eq!(rows, 1, "one row of the journal for the one record");
    desk.shutdown();
}

#[test]
fn what_is_done_is_not_said_twice_where_the_agent_waits_with_words_of_its_own() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let done = ["state=done:app=claude-code".to_string()];
    desk.window(&me, &reporting(&done)).unwrap();
    assert_eq!(shown(&desk).len(), 1);
    let call = Call {
        from: Some("t1"),
        ..mine.call()
    };
    let asked = json!({"action": "interact", "ask": "Claude finished: the cart is fixed"});
    desk.window(&call, &asked).unwrap();
    let events = shown(&desk);
    let titles: Vec<&str> = events.iter().map(|event| event.title.as_str()).collect();
    assert_eq!(titles, ["Claude finished: the cart is fixed"]);
    desk.shutdown();
}
