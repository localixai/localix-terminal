//! Tests of `center`.

use std::time::Duration;

use serde_json::{Value, json};

use super::super::tests::{caller, desk, desk_on};
use super::super::{Call, Desk};
use super::*;
use crate::testing::Scratch;

/// What the center shows now, once the feed has told it all that changed.
impl Event {
    fn labels(&self) -> Vec<&str> {
        self.buttons.iter().map(|button| button.label).collect()
    }
}

fn shown(desk: &Desk) -> Vec<Event> {
    assert!(desk.feed.settled(Duration::from_secs(5)));
    desk.paint_center();
    crate::lock(&desk.center).shown.clone()
}

/// The agent acts on its board, opened as a tab.
fn on_board(desk: &Desk, me: &Call, args: Value) -> String {
    desk.window(me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let mut args = args;
    args["action"] = json!("interact");
    args["tab"] = json!("b1");
    desk.window(me, &args).unwrap()
}

#[test]
fn a_card_handed_in_waits_for_the_person_who_accepts_it_from_the_center() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    on_board(&desk, &me, json!({"op": "add", "title": "Cart"}));
    on_board(&desk, &me, json!({"op": "take", "card": "c1"}));
    assert!(shown(&desk).is_empty());
    on_board(
        &desk,
        &me,
        json!({"op": "review", "card": "c1", "note": "fixed; checked at iPhone SE"}),
    );

    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let review = &events[0];
    assert_eq!(
        (review.kind, review.tab.as_str(), review.title.as_str()),
        (Kind::Review, "b1", "Cart")
    );
    assert!(
        review.detail.starts_with("agent: fixed"),
        "{}",
        review.detail
    );
    assert_eq!(review.labels(), ["Accept", "Open Board", "Dismiss"]);
    assert!(review.unseen);

    // Opened: seen, and the bell counts it no more.
    desk.center_seen();
    assert!(!shown(&desk)[0].unseen);

    // Accept takes the board's own path: the card is Done, and the event is gone.
    desk.center_do(review.n, 0);
    assert!(shown(&desk).is_empty());
    let read = on_board(&desk, &me, json!({}));
    assert!(read.contains("Done (1"), "{read}");
    desk.shutdown();
}

#[test]
fn a_blocked_card_is_unblocked_from_the_center_and_a_dismissed_one_comes_back_when_it_changes() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    on_board(&desk, &me, json!({"op": "add", "title": "Pay"}));
    on_board(&desk, &me, json!({"op": "take", "card": "c1"}));
    let block = json!({"op": "block", "card": "c1", "reason": "Stripe or Adyen?"});
    on_board(&desk, &me, block);

    let events = shown(&desk);
    let blocked = &events[0];
    assert_eq!(blocked.kind, Kind::Blocked);
    assert_eq!(blocked.detail, "Blocked: Stripe or Adyen?");
    assert_eq!(blocked.labels(), ["Unblock", "Open Board", "Dismiss"]);

    // Dismissed: gone while it stays as it is.
    desk.center_do(blocked.n, 2);
    assert!(shown(&desk).is_empty());
    on_board(
        &desk,
        &me,
        json!({"op": "comment", "card": "c1", "note": "waiting"}),
    );
    assert!(shown(&desk).is_empty());

    // Blocked for something else: news again, and Unblock takes the board's path.
    on_board(&desk, &me, json!({"op": "unblock", "card": "c1"}));
    let block = json!({"op": "block", "card": "c1", "reason": "The key, please"});
    on_board(&desk, &me, block);
    let events = shown(&desk);
    assert_eq!(events[0].detail, "Blocked: The key, please");
    desk.center_do(events[0].n, 0);
    assert!(shown(&desk).is_empty());
    desk.shutdown();
}

#[test]
fn a_program_that_ended_is_news_until_its_tab_is_closed_from_the_center() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    // It ends after its tab is open: one that ends while it opens is answered, and has no tab.
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 1; exit 3", "yield_ms": 200});
    desk.window(&me, &open).unwrap();
    let session = desk.session(1).unwrap();
    let _ = session.wait(None, 0, Duration::from_secs(5));
    assert!(session.ended());

    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let ended = &events[0];
    assert_eq!(
        (ended.kind, ended.tab.as_str(), ended.detail.as_str()),
        (Kind::Failed, "t1", "Exited with 3")
    );
    assert_eq!(ended.labels(), ["Show", "Close", "Dismiss"]);

    desk.center_do(ended.n, 1);
    assert!(shown(&desk).is_empty());
    assert_eq!(
        desk.window(&me, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
    desk.shutdown();
}

#[test]
fn the_agent_asks_the_person_on_a_tab_once_and_their_done_reaches_it_as_a_note() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&me, &open).unwrap();
    let ask = |words: &str| {
        let args = json!({"action": "interact", "tab": "t1", "ask": words, "yield_ms": 100});
        desk.window(&me, &args).unwrap()
    };

    let asked = ask("Sign in with your account");
    assert!(asked.starts_with("Asked the person on t1"), "{asked}");
    // One a tab: another replaces it.
    ask("Type the code from the SMS");
    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let event = &events[0];
    assert_eq!(
        (event.kind, event.title.as_str(), event.look.urgent),
        (Kind::Ask, "Type the code from the SMS", true)
    );
    assert_eq!(event.labels(), ["Done", "Show", "Dismiss"]);

    // Done: the agent hears it with its next call, and the ask goes.
    desk.center_do(event.n, 0);
    assert!(shown(&desk).is_empty());
    let said = desk.said(&mine.owner).unwrap();
    assert!(
        said.contains("The person did what you asked on t1: Type the code from the SMS"),
        "{said}"
    );

    // Taken back by the agent, and nothing to take back after.
    ask("Pick a plan");
    assert!(ask("").starts_with("The ask on t1 is taken back"));
    assert!(shown(&desk).is_empty());
    assert!(ask("").starts_with("Nothing was asked on t1"));

    // A tab closed takes its ask with it.
    ask("Pick a plan");
    desk.window(&me, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(shown(&desk).is_empty());
    assert!(crate::lock(&desk.asks).is_empty());
    desk.shutdown();
}

#[test]
fn a_session_that_waits_for_the_person_is_one_line_replaced_and_taken_back() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let wait = |words: &str| {
        let args = json!({"action": "interact", "ask": words});
        desk.window(&me, &args).unwrap()
    };

    let said = wait("Claude finished: the cart is fixed");
    assert!(
        said.starts_with("Asked the person for this session"),
        "{said}"
    );
    wait("Claude finished: tests pass");
    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let event = &events[0];
    assert_eq!(
        (
            event.kind,
            event.tab.as_str(),
            event.title.as_str(),
            event.look.urgent
        ),
        (Kind::Waiting, "", "Claude finished: tests pass", true)
    );
    assert_eq!(event.labels(), ["Dismiss"]);

    // It goes on: the line goes.
    assert!(wait("").starts_with("The ask for this session is taken back"));
    assert!(shown(&desk).is_empty());
    desk.shutdown();
}

#[test]
fn clearing_the_center_dismisses_all_it_shows_until_each_changes() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let ask = |words: &str| {
        let args = json!({"action": "interact", "ask": words});
        desk.window(&me, &args).unwrap();
    };
    ask("Claude finished: the cart is fixed");
    assert_eq!(shown(&desk).len(), 1);

    // Dismissed, the empty list is painted: the window never keeps what was cleared.
    let ids = desk.center_shown();
    desk.mark(&ids, journal::Mark::Dismissed);
    assert_eq!(desk.regathered().map(|events| events.len()), Some(0));
    assert!(shown(&desk).is_empty());
    // What changes after is shown again.
    ask("Claude finished: tests pass");
    assert_eq!(shown(&desk).len(), 1);
    desk.shutdown();
}

#[test]
fn the_journal_keeps_what_the_person_saw_across_a_restart_and_what_settled_as_history() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let first = desk_on(Arc::clone(&store));
    let mine = caller(&first, scratch.path(), None);
    let me = mine.call();
    on_board(&first, &me, json!({"op": "add", "title": "Cart"}));
    on_board(&first, &me, json!({"op": "take", "card": "c1"}));
    on_board(
        &first,
        &me,
        json!({"op": "review", "card": "c1", "note": "fixed; checked"}),
    );
    assert!(shown(&first)[0].unseen);
    first.center_seen();
    first.shutdown();
    assert!(first.feed.ended(Duration::from_secs(5)));

    // The desk again, on the same store: the card still waits, and is still seen.
    let again = desk_on(Arc::clone(&store));
    on_board(&again, &me, json!({"op": "look"}));
    let events = shown(&again);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(!events[0].unseen, "seen before the restart");

    // Accepted: it settles, and the journal keeps it.
    let posted = json!({"op": "accept", "card": "c1"}).to_string();
    again.person_on_board(mine.project.id, &posted);
    assert!(shown(&again).is_empty());
    let settled: u64 = store
        .read(|db| {
            db.query_row(
                "SELECT COUNT(*) FROM center_events WHERE settled IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|why| why.to_string())
        })
        .unwrap();
    assert_eq!(settled, 1);
    again.shutdown();
}

#[test]
fn an_id_seen_again_after_it_settled_or_in_another_project_is_a_new_event() {
    let store = crate::store::Store::memory().unwrap();
    let (one, two) = (Scratch::new(), Scratch::new());
    let (a, b) = store
        .write(|tx| {
            let a = crate::projects::find_or_make(tx, one.path(), 1)?;
            let b = crate::projects::find_or_make(tx, two.path(), 1)?;
            Ok((a, b))
        })
        .unwrap();
    let ended = |project: &crate::projects::Project| {
        let group = crate::projects::key(project.id);
        Event::new("ended:t3".into(), Kind::Finished, &group, "t3")
    };
    let record =
        |events: &[Event], now: u64| store.write(|tx| journal::record(tx, events, now)).unwrap();
    record(&[ended(&a)], 10);
    let ids = vec!["ended:t3".to_string()];
    store
        .write(|tx| journal::mark(tx, &ids, journal::Mark::Dismissed))
        .unwrap();
    // While it is open, the mark stays.
    assert!(record(&[ended(&a)], 11)["ended:t3"].dismissed);
    // Of another project (ids count afresh each run), it is new.
    assert!(!record(&[ended(&b)], 12)["ended:t3"].dismissed);
    store
        .write(|tx| journal::mark(tx, &ids, journal::Mark::Dismissed))
        .unwrap();
    // Settled (gone), then seen again: new as well.
    record(&[], 13);
    let again = record(&[ended(&b)], 14);
    assert!(!again["ended:t3"].dismissed);
    assert_eq!(again["ended:t3"].first, 14);
}
