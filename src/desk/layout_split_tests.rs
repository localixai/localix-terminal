//! Tests of the splits the person makes: a new terminal beside one opens in its folder and takes
//! the keys; a session takes more terminals and keeps one stack of documents for the tab; a
//! document dragged beside a terminal joins it; a pane taken out is a tab of its own; and a tab
//! is made even.

use std::sync::Arc;

use serde_json::json;

use super::super::Call;
use super::super::tests::{Caller, caller, desk_on};
use crate::browser::Side;
use crate::layout::{self, Tree};
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

/// The tab pane `key` is in.
fn tab_of(desk: &super::Desk, key: &str) -> layout::Tab {
    let row = crate::lock(&desk.ledger)[key].0;
    (desk.feed.store())
        .read(|db| {
            let tab = layout::tab_of(db, row)?.unwrap();
            Ok(layout::tab(db, tab)?.unwrap())
        })
        .unwrap()
}

/// The keys of rows `rows`.
fn keys(desk: &super::Desk, rows: &[i64]) -> Vec<String> {
    let ledger = crate::lock(&desk.ledger).clone();
    let key = |row: &i64| {
        (ledger.iter())
            .find(|(_, (kept, _))| kept == row)
            .map(|(key, _)| key.clone())
            .unwrap()
    };
    rows.iter().map(key).collect()
}

/// `mine`'s call as the person makes it in the window.
fn person(mine: &Caller) -> Call<'_> {
    Call {
        person: true,
        ..mine.call()
    }
}

#[test]
fn a_new_terminal_beside_one_opens_in_its_folder_and_takes_the_keys() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("sub/keep", "");
    let mine = caller(&desk, scratch.path(), None);
    let opened = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "workdir": "sub", "yield_ms": 200});
    desk.window(&mine.call(), &opened).unwrap();
    let folder = scratch.path().join("sub").canonicalize().unwrap();

    // ⌘D: a terminal on t1's right, in t1's folder, the pane in focus.
    let right = desk
        .split_new_told(&person(&mine), "t1", Side::Right, "sleep 30")
        .unwrap();
    let tab = tab_of(&desk, "t1");
    assert_eq!(keys(&desk, &tab.tree.visible()), ["t1", right.as_str()]);
    assert!(matches!(tab.tree, Tree::Branch { across: true, .. }));
    assert_eq!(
        keys(&desk, &tab.focused.into_iter().collect::<Vec<_>>()),
        [right.as_str()]
    );
    // ⇧⌘D from that one: a third under it, the same folder.
    let down = desk
        .split_new_told(&person(&mine), &right, Side::Down, "sleep 30")
        .unwrap();
    let tab = tab_of(&desk, "t1");
    assert_eq!(tab.tree.panes().len(), 3);
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;
    assert_eq!(tab.tree.path_of(&row(&down)), Some(vec![1, 1]));
    for key in [&right, &down] {
        let kept = (desk.feed.store())
            .read(|db| crate::ledger::get(db, row(key)))
            .unwrap()
            .unwrap();
        assert_eq!(kept.cwd.as_deref(), Some(folder.as_path()), "{key}");
        let id: u32 = key.trim_start_matches('t').parse().unwrap();
        let session = desk.session(id).unwrap();
        assert_eq!(session.folder().as_deref(), Some(folder.as_path()), "{key}");
    }
    desk.shutdown();
}

#[test]
fn a_session_takes_more_terminals_and_the_tab_keeps_one_stack_of_documents() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("a.rs", "fn main() {}\n");
    scratch.file("b.rs", "fn main() {}\n");
    let mine = caller(&desk, scratch.path(), None);
    let open = |call: &Call<'_>, args: serde_json::Value| desk.window(call, &args).unwrap();
    open(&mine.call(), serde_json::from_str(TERMINAL).unwrap());
    let from = |key| Call {
        from: Some(key),
        ..mine.call()
    };
    open(
        &from("t1"),
        json!({"action": "open", "kind": "file", "path": "a.rs"}),
    );
    // A second terminal in the session: beside t1, the documents still the tab's one stack.
    let two = desk
        .split_new_told(&person(&mine), "t1", Side::Right, "sleep 30")
        .unwrap();
    let tab = tab_of(&desk, "t1");
    assert_eq!(keys(&desk, &tab.tree.visible()), ["t1", two.as_str(), "f1"]);
    // What is opened from the second terminal joins the same stack.
    open(
        &from(&two),
        json!({"action": "open", "kind": "file", "path": "b.rs"}),
    );
    assert_eq!(
        keys(&desk, &tab_of(&desk, "t1").tree.stacked()),
        ["f1", "f2"]
    );
    // A terminal alone in its tab carried beside the session joins it too.
    open(&mine.call(), serde_json::from_str(TERMINAL).unwrap());
    desk.split_told("t3", "t1", Side::Down);
    assert!(keys(&desk, &tab_of(&desk, "t1").tree.panes()).contains(&"t3".to_string()));
    // Taken out, a terminal is a tab of its own after the session, which keeps the rest.
    desk.whole_told(&two);
    assert_eq!(
        keys(&desk, &tab_of(&desk, &two).tree.panes()),
        [two.as_str()]
    );
    let session = tab_of(&desk, "t1");
    assert_eq!(keys(&desk, &session.tree.stacked()), ["f1", "f2"]);
    assert!(!session.tree.holds(&crate::lock(&desk.ledger)[&two].0));
    desk.shutdown();
}

#[test]
fn a_document_put_beside_a_terminal_joins_it_and_a_session_takes_no_board() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("a.rs", "fn main() {}\n");
    let mine = caller(&desk, scratch.path(), None);
    let open = |args: serde_json::Value| desk.window(&mine.call(), &args).unwrap();
    open(serde_json::from_str(TERMINAL).unwrap());
    open(serde_json::from_str(TERMINAL).unwrap());
    open(json!({"action": "open", "kind": "file", "path": "a.rs"}));
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;

    // The person looks at the loose file and carries a terminal beside it, on its right: the
    // file is that terminal's document now, and still the pane in use.
    desk.split_told("t1", "f1", Side::Right);
    let tab = tab_of(&desk, "t1");
    assert_eq!(tab.tree.visible(), [row("f1"), row("t1")]);
    assert_eq!(tab.tree.stacked(), [row("f1")]);
    assert_eq!(tab.focused, Some(row("f1")));

    // The board carried beside that session is refused: the session is as it was, and the
    // board still a tab of its own.
    open(json!({"action": "open", "kind": "board"}));
    desk.split_told("b1", "t1", Side::Right);
    assert_eq!(tab_of(&desk, "t1").tree, tab.tree);
    assert_eq!(tab_of(&desk, "b1").tree, Tree::leaf(row("b1")));
    desk.shutdown();
}

#[test]
fn a_tab_made_even_shares_by_what_each_child_holds() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    for _ in 0..3 {
        desk.window(&mine.call(), &serde_json::from_str(TERMINAL).unwrap())
            .unwrap();
    }
    desk.split_told("t2", "t1", Side::Right);
    desk.split_told("t3", "t2", Side::Down);
    desk.resized("t1", &[], 1, 8000).unwrap();
    desk.equalized("t3").unwrap();
    let Tree::Branch { children, .. } = tab_of(&desk, "t1").tree else {
        panic!("not a split")
    };
    // t1 lays one pane across, the column of two one too: half each.
    let shares: Vec<f64> = children.iter().map(|child| child.share).collect();
    assert!(
        shares.iter().all(|share| (share - 0.5).abs() < 1e-9),
        "{shares:?}"
    );
    desk.shutdown();
}
