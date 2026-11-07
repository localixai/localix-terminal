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
    // screen above; the same pane, in the same tab.
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
    assert_eq!(left[0].row, old);
    let tab = store.read(|db| crate::layout::tab_of(db, old)).unwrap();
    assert!(tab.is_some());
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
    // Opened again: a terminal in its folder, the same row open again, in a tab of its own.
    let group = crate::projects::key(mine.project.id);
    desk.again(&mine.call(), &group, past[0].row).unwrap();
    let past = store
        .read(|db| ledger::past(db, mine.project.id, 10))
        .unwrap();
    assert_eq!(past.len(), 1);
    assert!(past[0].closed.is_none());
    let tab = store
        .read(|db| crate::layout::tab_of(db, past[0].row))
        .unwrap();
    assert!(tab.is_some());
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

#[test]
fn a_screen_is_kept_with_the_secrets_it_shows_masked() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let dir = scratch.path().join("secrets");
    crate::secrets::save_in(&dir, "PW", "hunter2-long").unwrap();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    // A call typed the secret in, and the program echoed it.
    desk.vault.fill_in(Some(&dir), "{{secret:PW}}").unwrap();
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'echo pw=hunter2-long; sleep 30'", "wait_for": "pw="});
    desk.window(&mine.call(), &open).unwrap();
    desk.shutdown();
    let left = store
        .read(|db| ledger::left_open(db, mine.project.id))
        .unwrap();
    let kept = store
        .read(|db| ledger::screen(db, left[0].row))
        .unwrap()
        .unwrap();
    assert!(kept.contains("pw={{secret:PW}}"), "{kept}");
    assert!(!kept.contains("hunter2-long"), "{kept}");
}

#[test]
fn a_title_said_often_is_written_once_in_a_while_and_its_last_word_when_the_row_closes() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "yield_ms": 200});
    desk.window(&mine.call(), &open).unwrap();
    let title = || {
        let past = store
            .read(|db| ledger::past(db, mine.project.id, 10))
            .unwrap();
        past[0].title.clone()
    };
    desk.retitled("t1", "one");
    desk.retitled("t1", "two");
    desk.retitled("t1", "three");
    assert_eq!(title(), "one");
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert_eq!(title(), "three");
    desk.shutdown();
}
