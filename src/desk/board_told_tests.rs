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
        who: None,
    };
    let loud = Did {
        words: "commented on c3 \"API\": \"why\"".into(),
        urgent: Urgency::Now,
        who: None,
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
        again: None,
        person: false,
        window: None,
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

#[test]
fn quiet_news_of_the_board_waits_for_a_call_and_is_not_told_again_to_who_subscribes() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    // The person adds a card and runs nothing: news that waits. A session that starts after it
    // and subscribes is not woken with it as if it had just been said.
    desk.person_on_board(mine.project.id, r#"{"op": "add", "title": "Header"}"#);
    desk.tell_board(mine.project.id);
    assert!(desk.pending(&mine.owner).is_empty());
    // What wants acting on now is told again to who subscribes, until it is taken.
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "comment", "card": "c1", "text": "Make it sticky"}"#,
    );
    desk.tell_board(mine.project.id);
    let again = desk.pending(&mine.owner);
    assert_eq!(again.len(), 1, "{again:?}");
    assert!(again[0].text.contains("Make it sticky"), "{again:?}");
    // The next call carries both, the quiet news with it.
    let said = desk.call(&me, &json!({"action": "list"})).unwrap().said;
    let said = said.unwrap_or_default();
    assert!(
        said.contains("added c1 \"Header\"") && said.contains("Make it sticky"),
        "{said}"
    );
    assert!(desk.pending(&mine.owner).is_empty());
}

#[test]
fn a_card_dropped_where_it_was_is_no_news_to_the_agent() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let added = json!({"action": "interact", "tab": "b1", "op": "add",
        "cards": [{"title": "Cart"}, {"title": "Header"}]});
    desk.window(&me, &added).unwrap();
    // The last card of Todo dropped at the bottom of Todo again.
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "move", "card": "c2", "to": "todo"}"#,
    );
    desk.tell_board(mine.project.id);
    let read = desk
        .window(&me, &json!({"action": "interact", "tab": "b1"}))
        .unwrap();
    assert!(!read.contains("The person on the board"), "{read}");
}

/// The plugin routes a note to the worker whose card it is by this pattern (`NOTE` in its hooks
/// module, read from there so there is one copy): every note the formatter makes about a worker's
/// card has to match it, or the note reaches the main agent alone.
#[test]
fn the_notes_about_a_workers_card_are_what_the_plugin_routes_on() {
    use crate::board::{Board, Worker};

    let plugin = include_str!("../../plugins/claude-code/hooks/register.tsx");
    let pattern = plugin
        .lines()
        .find_map(|line| line.strip_prefix("const NOTE = /"))
        .and_then(|line| line.strip_suffix('/'))
        .unwrap();
    assert!(
        pattern.starts_with(r"^The person on the board \S+ (?:"),
        "{pattern}"
    );
    assert!(
        pattern.contains(r#" c\d+ "(?:[^"\\]|\\.)*" \(([\w.:-]+)\)(?: |:|\n|$)"#),
        "{pattern}"
    );
    let verbs: Vec<&str> = pattern
        .split_once("(?:")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(verbs, _)| verbs.split('|').collect())
        .unwrap();
    for action in [
        json!({"op": "return", "card": "c1", "text": "again"}),
        json!({"op": "comment", "card": "c1", "text": "why"}),
        json!({"op": "edit", "card": "c1", "title": "uno"}),
        json!({"op": "move", "card": "c1", "to": "review"}),
        json!({"op": "unblock", "card": "c1"}),
    ] {
        let mut board = Board::default();
        board.add("one", "", None, "agent").unwrap();
        board.take(Some("c1"), Worker(Some("tests"))).unwrap();
        let did = board.person(&action).unwrap();
        let (words, _) = note("b1", vec![did]).unwrap();
        let said = words.strip_prefix("The person on the board b1 ").unwrap();
        let verb = verbs
            .iter()
            .find(|verb| said.starts_with(&format!("{verb} c1 \"")))
            .unwrap_or_else(|| panic!("the plugin does not know the verb of {words:?}"));
        let rest = said
            .strip_prefix(*verb)
            .unwrap()
            .split_once(" (tests)")
            .unwrap_or_else(|| {
                panic!("no owner in {words:?}");
            });
        assert!(
            rest.1.is_empty() || rest.1.starts_with([' ', ':', '\n']),
            "the owner is followed by something the pattern does not take: {words:?}"
        );
    }
}
