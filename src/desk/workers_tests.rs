//! Tests of `workers`: a card's worker in a terminal of its own, and what it alone is told.

use serde_json::{Value, json};

use super::super::tests::{caller, desk};
use crate::desk::Call;
use crate::testing::Scratch;

#[test]
fn what_the_person_says_of_a_card_goes_to_the_worker_that_runs_it_while_it_runs() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let board = |args: Value| desk.window(&me, &args).unwrap();
    board(json!({"action": "open", "kind": "board"}));
    board(json!({"action": "interact", "tab": "b1", "op": "add", "cards": [{"title": "Cart"}]}));
    // The person runs the card: its worker's terminal, and the card the worker's.
    let theirs = Call {
        person: true,
        window: None,
        ..mine.call()
    };
    let key = (desk.worker_terminal(&theirs, "1", "sleep 30", "c1 · sleep", "claude-c1")).unwrap();
    board(
        json!({"action": "interact", "tab": "b1", "op": "take", "card": "c1", "who": "claude-c1"}),
    );
    assert_eq!(
        desk.worker_in(Some(&key), mine.project.id).as_deref(),
        Some("claude-c1")
    );
    assert_eq!(desk.worker_in(Some("t9"), mine.project.id), None);

    // The worker is an agent of the project by its name: the board is its project's too.
    let worker = caller(&desk, scratch.path(), Some("claude-c1"));
    // Its name is its name on the board: it works its card with no `who` of its own to pass.
    let read = json!({"action": "interact", "tab": "b1", "op": "card", "card": "c1"});
    let card = desk.window(&worker.call(), &read).unwrap();
    assert!(card.contains("Cart"), "{card}");
    let block = json!({"action": "interact", "tab": "b1", "op": "block", "card": "c1", "reason": "Which font?"});
    desk.window(&worker.call(), &block).unwrap();
    let unblock = json!({"action": "interact", "tab": "b1", "op": "unblock", "card": "c1"});
    desk.window(&worker.call(), &unblock).unwrap();

    // A comment on its card is the worker's alone; a card added is the project's agent's.
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "comment", "card": "c1", "text": "Make it sticky"}"#,
    );
    desk.person_on_board(mine.project.id, r#"{"op": "add", "title": "Header"}"#);
    desk.tell_board(mine.project.id);
    let told = desk.pending(&worker.owner);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].text.contains("Make it sticky") && !told[0].text.contains("Header"));
    assert!(desk.pending(&mine.owner).is_empty());
    let said = desk.said(&mine.owner).unwrap_or_default();
    assert!(
        said.contains("Header") && !said.contains("Make it sticky"),
        "{said}"
    );

    // Its terminal over, what is said of the card is the project's agent's again.
    desk.window(&me, &json!({"action": "close", "tab": key}))
        .unwrap();
    desk.person_on_board(
        mine.project.id,
        r#"{"op": "comment", "card": "c1", "text": "And the footer"}"#,
    );
    desk.tell_board(mine.project.id);
    let told = desk.pending(&mine.owner);
    assert!(
        told.len() == 1 && told[0].text.contains("And the footer"),
        "{told:?}"
    );
    desk.shutdown();
}
