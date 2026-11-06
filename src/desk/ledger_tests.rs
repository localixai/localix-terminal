//! Tests of the desk's ledger: tabs kept by project, closed into its history, and opened again
//! as they were left.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{caller, desk_on};
use crate::ledger;
use crate::testing::Scratch;

#[test]
fn a_project_opens_again_as_it_was_left_its_terminals_screen_shown() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let first = desk_on(Arc::clone(&store));
    let mine = caller(&first, scratch.path(), None);
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'echo kept-line; sleep 30'", "wait_for": "kept-line"});
    first.window(&mine.call(), &open).unwrap();
    let project = mine.project.id;
    let left = store.read(|db| ledger::left_open(db, project)).unwrap();
    assert_eq!(left.len(), 1, "{left:?}");
    // The app quits: the terminal stays open in the ledger, its screen kept.
    first.shutdown();
    let left = store.read(|db| ledger::left_open(db, project)).unwrap();
    assert_eq!(left.len(), 1, "{left:?}");
    let old = left[0].row;
    let kept = store.read(|db| ledger::screen(db, old)).unwrap();
    assert!(kept.is_some_and(|text| text.contains("kept-line")));

    // The app again: the project opens as it was, a shell where the terminal was, its last
    // screen above; the row it was is gone, the new one open.
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    again
        .reopen_project(&mine.call(), &crate::projects::key(project))
        .unwrap();
    let session = crate::lock(&again.sessions)
        .values()
        .next()
        .cloned()
        .unwrap();
    assert!(session.text().contains("kept-line"), "{}", session.text());
    let left = store.read(|db| ledger::left_open(db, project)).unwrap();
    assert_eq!(left.len(), 1);
    assert_ne!(left[0].row, old);
    assert!(store.read(|db| ledger::get(db, old)).unwrap().is_none());
    again.shutdown();
}

#[test]
fn a_closed_tab_is_the_projects_history_and_opens_again_from_it() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "yield_ms": 200});
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    let past = store
        .read(|db| ledger::past(db, mine.project.id, 10))
        .unwrap();
    assert!(past[0].closed.is_some(), "{past:?}");
    // Another project's history is its own.
    let other = Scratch::new();
    let theirs = caller(&desk, other.path(), None);
    let none = store
        .read(|db| ledger::past(db, theirs.project.id, 10))
        .unwrap();
    assert!(none.is_empty());
    // Opened again: a terminal in its folder, the closed row gone from the history.
    let group = crate::projects::key(mine.project.id);
    desk.again(&mine.call(), &group, past[0].row).unwrap();
    let past = store
        .read(|db| ledger::past(db, mine.project.id, 10))
        .unwrap();
    assert_eq!(past.len(), 1);
    assert!(past[0].closed.is_none());
    desk.shutdown();
}

#[test]
fn what_another_run_left_open_is_past_once_the_project_has_a_tab_this_run() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "yield_ms": 200});
    let first = desk_on(Arc::clone(&store));
    let mine = caller(&first, scratch.path(), None);
    first.window(&mine.call(), &open).unwrap();
    let project = mine.project.id;
    first.shutdown();
    let old = store.read(|db| ledger::left_open(db, project)).unwrap()[0].row;

    // An agent opens a tab before the person ever goes back: the old one is history now, so
    // closing every tab of this run never brings it back.
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    again.window(&mine.call(), &open).unwrap();
    let left = store.read(|db| ledger::left_open(db, project)).unwrap();
    assert!(left.iter().all(|tab| tab.row != old), "{left:?}");
    let past = store.read(|db| ledger::past(db, project, 10)).unwrap();
    assert!(past.iter().any(|tab| tab.row == old), "{past:?}");
    again.shutdown();
}
