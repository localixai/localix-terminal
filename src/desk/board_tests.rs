//! Tests of `board`: the `board` action as a worker calls it, over a desk with no window.

use serde_json::json;

use super::super::tests::{caller, desk, written};
use crate::testing::Scratch;

#[test]
fn a_worker_plans_takes_and_hands_in_cards_and_the_board_is_kept_in_the_project() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let added = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "add", "cards": [
            {"title": "Header", "note": "logo left"}, {"title": "Footer"}]}),
        )
        .unwrap();
    assert!(
        added.starts_with("b1: added c1, c2 to the bottom of Todo."),
        "{added}"
    );
    assert!(
        added.contains("Todo (2, top first):\n  c1 Header\n  c2 Footer"),
        "{added}"
    );
    let took = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "take"}),
        )
        .unwrap();
    assert!(took.contains("took c1: it is agent's"), "{took}");
    assert!(
        took.contains("Note:\nlogo left"),
        "the card is read whole: {took}"
    );
    let refused = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "take"}),
        )
        .unwrap_err();
    assert!(
        refused.message().contains("agent already has c1 in Doing"),
        "{refused:?}"
    );
    let handed = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "review", "card": "c1", "note": "done"}),
        )
        .unwrap();
    assert!(
        handed.contains("Review, waiting for the person (1):"),
        "{handed}"
    );
    let saved = written(&desk, &scratch.path().join(".localix/board.json"));
    assert!(saved.contains("\"column\": \"review\""), "{saved}");
    let listed = desk.window(&me, &json!({"action": "list"})).unwrap();
    assert_eq!(
        listed,
        "b1 (board): 2 card(s) · 0 in Doing · 1 waiting for the person"
    );
}

#[test]
fn a_subagent_acts_on_its_own_cards_and_holds_one_at_a_time() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let board = |args: serde_json::Value| desk.window(&me, &args);
    board(json!({"action": "interact", "tab": "b1", "op": "add", "cards": [{"title": "a"}, {"title": "b"}]}))
        .unwrap();
    let took =
        board(json!({"action": "interact", "tab": "b1", "op": "take", "who": "tests"})).unwrap();
    assert!(took.contains("took c1: it is tests's"), "{took}");
    let other = board(
        json!({"action": "interact", "tab": "b1", "op": "review", "card": "c1", "who": "api", "note": "x"}),
    );
    assert!(other.unwrap_err().message().contains("tests's"));
    let second =
        board(json!({"action": "interact", "tab": "b1", "op": "take", "who": "api"})).unwrap();
    assert!(second.contains("took c2"), "{second}");
    assert!(
        second
            .ends_with("api: c2 in Doing; hand it in with `review` and a `note` of what was done.")
    );
    let bad = board(json!({"action": "interact", "tab": "b1", "who": "two words"})).unwrap_err();
    assert!(bad.message().contains("short name"));
    let unknown = board(json!({"action": "interact", "tab": "b1", "op": "fly"})).unwrap_err();
    assert!(unknown.message().contains("no board op `fly`"));
}

#[test]
fn a_board_a_project_kept_in_its_file_is_taken_in_once_and_the_file_follows_the_board() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let mut before = crate::board::Board::default();
    before.add("kept before", "", None, "person").unwrap();
    let path = scratch.file(
        ".localix/board.json",
        serde_json::to_string(&before).unwrap(),
    );
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let read = desk
        .window(&me, &json!({"action": "interact", "tab": "b1"}))
        .unwrap();
    assert!(read.contains("c1 kept before"), "{read}");

    // The file is written out from the board: an edit by hand is not read, and is written over.
    std::fs::write(&path, "{").unwrap();
    desk.window(
        &me,
        &json!({"action": "interact", "tab": "b1", "op": "add", "title": "mine"}),
    )
    .unwrap();
    let saved = written(&desk, &path);
    assert!(
        saved.contains("kept before") && saved.contains("mine"),
        "{saved}"
    );
    desk.shutdown();
}

#[test]
fn a_board_tab_is_read_as_text_closed_and_opened_again_with_its_cards() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let opened = desk
        .window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    assert!(opened.starts_with("board: b1"), "{opened}");
    desk.window(
        &me,
        &json!({"action": "interact", "tab": "b1", "op": "add", "title": "kept"}),
    )
    .unwrap();
    let shot = desk
        .window(&me, &json!({"action": "screenshot", "tab": "b1"}))
        .unwrap();
    assert!(
        shot.contains("A board is read as text") && shot.contains("c1 kept"),
        "{shot}"
    );
    let acted = desk.window(
        &me,
        &json!({"action": "interact", "tab": "b1", "keys": ["Enter"]}),
    );
    assert!(
        acted
            .unwrap_err()
            .message()
            .contains("acted on with an `op`")
    );
    let closed = desk
        .window(&me, &json!({"action": "close", "tab": "b1"}))
        .unwrap();
    assert!(
        closed.starts_with("Closed b1; the board is kept"),
        "{closed}"
    );
    assert_eq!(
        desk.window(&me, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
    // A closed board is no tab of the agent's until it opens it again, with its cards.
    let gone = desk.window(&me, &json!({"action": "interact", "tab": "b1"}));
    assert!(gone.unwrap_err().message().contains("no tab b1"));
    let again = desk
        .window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    assert!(
        again.starts_with("board: b2") && again.contains("c1 kept"),
        "{again}"
    );
}

#[test]
fn taking_a_card_the_person_moved_to_doing_says_it_was_theirs_already() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    desk.window(
        &me,
        &json!({"action": "interact", "tab": "b1", "op": "add", "title": "Hello"}),
    )
    .unwrap();
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "move", "card": "c1", "to": "doing"}"#,
    );
    let took = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "take", "card": "c1"}),
        )
        .unwrap();
    assert!(
        took.starts_with("b1: c1 is yours already: the person moved it to Doing"),
        "{took}"
    );
}

#[test]
fn the_agent_writes_an_idea_down_in_backlog_and_takes_none_from_there() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let added = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "add", "title": "Dark mode", "to": "backlog"}),
        )
        .unwrap();
    assert!(
        added.starts_with("b1: added c1 to the bottom of Backlog."),
        "{added}"
    );
    let taken = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "take"}),
        )
        .unwrap_err();
    assert!(matches!(taken, crate::desk::Failure::Failed(why) if why.contains("Todo is empty")));
}

#[test]
fn a_batch_add_that_fails_midway_leaves_the_board_as_it_was() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let refused = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "add", "cards": [
            {"title": "One"}, {"title": "Two"}, {"note": "no title"}]}),
        )
        .unwrap_err();
    assert!(refused.message().contains("has a `title`"), "{refused:?}");
    // Sent again whole, the cards are not twice on it.
    let added = desk
        .window(
            &me,
            &json!({"action": "interact", "tab": "b1", "op": "add", "cards": [
            {"title": "One"}, {"title": "Two"}, {"title": "Three"}]}),
        )
        .unwrap();
    assert!(added.contains("added c1, c2, c3"), "{added}");
}

#[test]
fn a_second_agent_acts_on_the_projects_board_tab_it_was_given() {
    let (desk, scratch) = (desk(), Scratch::new());
    let cart = caller(&desk, scratch.path(), Some("cart"));
    let header = caller(&desk, scratch.path(), Some("header"));
    desk.window(&cart.call(), &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let opened = desk
        .window(&header.call(), &json!({"action": "open", "kind": "board"}))
        .unwrap();
    assert!(opened.contains("b1"), "{opened}");
    let added = json!({"action": "interact", "tab": "b1", "op": "add", "title": "Header"});
    assert!(
        desk.window(&header.call(), &added)
            .unwrap()
            .contains("added c1")
    );
    // Another project's board is none of theirs.
    let other = Scratch::new();
    let stranger = caller(&desk, other.path(), Some("cart"));
    assert!(desk.window(&stranger.call(), &added).is_err());
}
