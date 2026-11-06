//! Tests of `board_told`: what the person does on a board, and what the agent hears of it.

use serde_json::json;

use super::super::tests::{caller, desk, written};
use super::*;
use crate::desk::Call;
use crate::testing::Scratch;

#[test]
fn what_the_person_does_is_applied_kept_and_told_with_the_agents_next_call() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let board = |args: Value| desk.window(&me, &args).unwrap();
    board(
        json!({"action": "interact", "tab": "b1", "op": "add", "cards": [{"title": "Cart"}, {"title": "Header"}]}),
    );
    board(json!({"action": "interact", "tab": "b1", "op": "take", "card": "c1", "who": "tests"}));
    board(
        json!({"action": "interact", "tab": "b1", "op": "review", "card": "c1", "who": "tests", "note": "fixed"}),
    );
    desk.person_on_board(mine.project.id, r#"{"op": "accept", "card": "c2"}"#);
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "return", "card": "c1", "text": "still overlaps"}"#,
    );
    desk.person_on_board(mine.project.id, "not json");
    desk.tell_board(mine.project.id);
    let read = board(json!({"action": "interact", "tab": "b1"}));
    assert!(
        read.contains(
            "The person on the board b1 returned c1 \"Cart\" (tests) to Todo: \"still overlaps\"\n\
             They also accepted c2 \"Header\""
        ),
        "the person's actions come as one note, what wants acting on first: {read}"
    );
    assert!(
        read.contains("c1 Cart · RETURNED just now: \"still overlaps\" · was tests's"),
        "{read}"
    );
    let saved = written(&desk, &scratch.path().join(".localix/board.json"));
    assert!(saved.contains("still overlaps"), "{saved}");
    // Told once: the next call hears nothing again.
    let again = board(json!({"action": "interact", "tab": "b1"}));
    assert!(!again.contains("The person on the board"), "{again}");
}

#[test]
fn a_note_puts_what_wants_acting_on_first_and_says_whether_to_tell_now() {
    let quiet = Did {
        words: "accepted c2 \"Header\"".into(),
        urgent: Urgency::Later,
    };
    let loud = Did {
        words: "commented on c3 \"API\": \"why\"".into(),
        urgent: Urgency::Now,
    };
    let (words, urgent) = note("b1", vec![quiet.clone(), loud]).unwrap();
    assert!(
        words.starts_with(
            "The person on the board b1 commented on c3 \"API\": \"why\"\nThey also accepted c2"
        ),
        "{words}"
    );
    assert_eq!(urgent, Urgency::Now);
    assert_eq!(note("b1", vec![quiet]).unwrap().1, Urgency::Later);
    assert_eq!(note("b1", vec![]), None);
}

#[test]
fn a_panes_look_leaves_the_persons_notes_for_the_agents_own_call() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    desk.window(
        &me,
        &json!({"action": "interact", "tab": "b1", "op": "add", "title": "Cart"}),
    )
    .unwrap();
    desk.person_on_board(mine.project.id, r#"{"op": "add", "title": "Header"}"#);
    desk.tell_board(mine.project.id);
    let peek = Call {
        peek: true,
        ..mine.call()
    };
    let looked = desk.call(&peek, &json!({"action": "list"})).unwrap();
    assert!(
        looked.said.is_none(),
        "a pane's look takes nothing: {:?}",
        looked.said
    );
    let mine = desk.call(&me, &json!({"action": "list"})).unwrap();
    assert!(
        mine.said
            .as_deref()
            .is_some_and(|said| said.contains("added c2 \"Header\"")),
        "the agent's own call still gets it: {:?}",
        mine.said
    );
}
