//! Tests of `board`: what a worker may do, and what it may not.

use super::*;

const MAIN: Worker<'static> = Worker(None);
const TESTS: Worker<'static> = Worker(Some("tests"));
const API: Worker<'static> = Worker(Some("api"));

fn board(titles: &[&str]) -> Board {
    let mut board = Board::default();
    for title in titles {
        board.add(title, "", None, "agent").unwrap();
    }
    board
}

fn column(board: &Board, column: Column) -> Vec<&str> {
    (board.cards.iter())
        .filter(|card| card.column == column)
        .map(|card| card.id.as_str())
        .collect()
}

#[test]
fn cards_are_added_to_the_bottom_of_todo_with_ids_never_used_again() {
    let mut board = board(&["one", "two"]);
    assert_eq!(column(&board, Column::Todo), ["c1", "c2"]);
    board.remove("c2", MAIN).unwrap();
    assert_eq!(board.add("three", "", Some("w1"), "agent").unwrap(), "c3");
    assert_eq!(board.card("C3").unwrap().tab.as_deref(), Some("w1"));
    assert!(
        board
            .add("  ", "", None, "agent")
            .unwrap_err()
            .contains("title")
    );
}

#[test]
fn take_without_an_id_takes_the_top_of_todo_and_a_worker_holds_one_at_a_time() {
    let mut board = board(&["one", "two", "three"]);
    assert_eq!(board.take(None, TESTS).unwrap(), "c1");
    assert_eq!(board.card("c1").unwrap().who.as_deref(), Some("tests"));
    // Taking what it holds again is no change.
    assert_eq!(board.take(Some("c1"), TESTS).unwrap(), "c1");
    let refused = board.take(None, TESTS).unwrap_err();
    assert!(
        refused.contains("tests already has c1 in Doing"),
        "{refused}"
    );
    // Another worker has a limit of its own.
    assert_eq!(board.take(None, API).unwrap(), "c2");
    assert_eq!(column(&board, Column::Doing), ["c1", "c2"]);
}

#[test]
fn a_card_one_worker_holds_is_not_taken_or_touched_by_another() {
    let mut board = board(&["one"]);
    board.take(Some("c1"), TESTS).unwrap();
    assert!(
        board
            .take(Some("c1"), API)
            .unwrap_err()
            .contains("being done by tests")
    );
    assert!(
        board
            .review("c1", API, "done")
            .unwrap_err()
            .contains("tests's")
    );
    assert!(board.release("c1", API, None).is_err());
    // The main agent acts on any card.
    board.comment("c1", MAIN, "how is it going?").unwrap();
}

#[test]
fn a_card_is_handed_in_from_doing_with_what_was_done_and_never_put_in_done() {
    let mut board = board(&["one"]);
    assert!(
        board
            .review("c1", MAIN, "done")
            .unwrap_err()
            .contains("from Doing")
    );
    board.take(Some("c1"), MAIN).unwrap();
    assert!(
        board
            .review("c1", MAIN, " ")
            .unwrap_err()
            .contains("what was done")
    );
    board.review("c1", MAIN, "fixed the overlap").unwrap();
    let card = board.card("c1").unwrap();
    assert_eq!(card.column, Column::Review);
    assert_eq!(card.result.as_deref(), Some("fixed the overlap"));
    // Handed in, it is not taken again: the person accepts it or sends it back.
    assert!(
        board
            .take(Some("c1"), MAIN)
            .unwrap_err()
            .contains("in review")
    );
}

#[test]
fn a_blocked_card_waits_for_the_person_and_frees_its_worker_to_take_another() {
    let mut board = board(&["one", "two"]);
    board.take(None, TESTS).unwrap();
    board
        .block("c1", TESTS, Some("waiting for the API key"))
        .unwrap();
    assert_eq!(board.take(None, TESTS).unwrap(), "c2");
    board.block("c1", TESTS, None).unwrap();
    assert_eq!(board.card("c1").unwrap().blocked, None);
    let history = &board.card("c1").unwrap().history;
    assert_eq!(history.last().unwrap().text.as_deref(), Some("unblocked"));
}

#[test]
fn a_released_card_goes_back_to_the_bottom_of_todo_for_anyone() {
    let mut board = board(&["one", "two"]);
    board.take(Some("c1"), TESTS).unwrap();
    board.release("c1", TESTS, None).unwrap();
    assert_eq!(column(&board, Column::Todo), ["c2", "c1"]);
    assert_eq!(board.card("c1").unwrap().who, None);
}

#[test]
fn a_card_released_with_a_note_keeps_where_it_stands_for_the_next_worker() {
    let mut board = board(&["one"]);
    board.take(Some("c1"), TESTS).unwrap();
    board
        .release("c1", TESTS, Some("layout done, Safari still fails"))
        .unwrap();
    let last = board.card("c1").unwrap().history.last().unwrap().clone();
    assert_eq!(last.by, "tests");
    assert_eq!(last.to, Some(Column::Todo));
    assert_eq!(
        last.text.as_deref(),
        Some("layout done, Safari still fails")
    );
    // A blank note is no note.
    board.take(Some("c1"), API).unwrap();
    board.release("c1", API, Some("  ")).unwrap();
    assert_eq!(board.card("c1").unwrap().history.last().unwrap().text, None);
}

#[test]
fn only_a_card_in_todo_is_removed_and_a_worker_names_itself_plainly() {
    let mut board = board(&["one"]);
    board.take(Some("c1"), MAIN).unwrap();
    assert!(
        board
            .remove("c1", MAIN)
            .unwrap_err()
            .contains("only a card in Todo")
    );
    assert_eq!(worker_name(Some(" tests ")).unwrap(), Some("tests"));
    assert_eq!(worker_name(Some("")).unwrap(), None);
    assert!(worker_name(Some("person")).is_err());
    assert!(worker_name(Some("two words")).is_err());
}

#[test]
fn columns_read_by_name_and_a_card_knows_when_it_came_to_its_own() {
    assert_eq!(Column::parse(" Review "), Some(Column::Review));
    assert_eq!(Column::parse("later"), None);
    let mut board = board(&["one"]);
    board.cards[0].history[0].at = 10;
    assert_eq!(board.card("c1").unwrap().since(), Some(10));
    board.comment("c1", MAIN, "a word").unwrap();
    assert_eq!(board.card("c1").unwrap().since(), Some(10));
}

#[test]
fn an_idea_waits_in_backlog_until_the_person_moves_it_to_todo() {
    let mut board = board(&["plan"]);
    let idea = board
        .add_to(Column::Backlog, "dark mode", "", None, "agent")
        .unwrap();
    assert_eq!(column(&board, Column::Backlog), ["c2"]);
    let refused = board.take(Some(&idea), MAIN).unwrap_err();
    assert!(refused.contains("an idea in Backlog"), "{refused}");
    // Taking the top of Todo passes the ideas by.
    assert_eq!(board.take(None, MAIN).unwrap(), "c1");
    let only = board
        .add_to(Column::Doing, "x", "", None, "agent")
        .unwrap_err();
    assert!(only.contains("Todo or to Backlog"), "{only}");
    // An idea is removed as a card in Todo is.
    assert_eq!(board.remove(&idea, MAIN).unwrap().id, "c2");
}

#[test]
fn a_cards_history_keeps_its_first_step_and_the_latest() {
    let mut one: Card =
        serde_json::from_value(serde_json::json!({"id": "c1", "title": "t", "column": "todo"}))
            .unwrap();
    for n in 0..(MAX_STEPS + 50) {
        said(&mut one, "me", &n.to_string());
    }
    assert_eq!(one.history.len(), MAX_STEPS);
    assert_eq!(one.history[0].text.as_deref(), Some("0"));
    assert_eq!(one.history[1].text.as_deref(), Some("51"));
    assert_eq!(
        one.history.last().unwrap().text.as_deref(),
        Some(&*(MAX_STEPS + 49).to_string())
    );
}
