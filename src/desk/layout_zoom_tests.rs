//! Tests of a tab's zoom (docs/windows.md): one pane alone over the tab's part, the rest kept
//! in its tree; another pane of the tab shown keeps it zoomed; a new split, a close, a divider
//! moved and the tab made even end it; a pane alone has nothing to zoom.

use std::sync::Arc;

use serde_json::json;

use crate::browser::Side;
use crate::desk::Call;
use crate::desk::tests::{Caller, caller, desk_on};
use crate::layout;
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

/// The tab pane `key` is in.
fn tab_of(desk: &crate::desk::Desk, key: &str) -> layout::Tab {
    let row = crate::lock(&desk.ledger)[key].0;
    (desk.feed.store())
        .read(|db| {
            let tab = layout::tab_of(db, row)?.unwrap();
            Ok(layout::tab(db, tab)?.unwrap())
        })
        .unwrap()
}

/// `mine`'s call as the person makes it in the window.
fn person(mine: &Caller) -> Call<'_> {
    Call {
        person: true,
        ..mine.call()
    }
}

/// Whether the tab of `key` is zoomed on `key`.
fn zoomed_on(desk: &crate::desk::Desk, key: &str) -> bool {
    let tab = tab_of(desk, key);
    tab.whole && tab.focused == Some(crate::lock(&desk.ledger)[key].0)
}

#[test]
fn a_zoom_moves_with_the_pane_shown_and_ends_with_a_split_a_close_or_a_divider() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    for _ in 0..3 {
        desk.window(&mine.call(), &serde_json::from_str(TERMINAL).unwrap())
            .unwrap();
    }
    desk.split_told("t2", "t1", Side::Right);
    desk.split_told("t3", "t2", Side::Down);
    let panes = tab_of(&desk, "t1").tree;

    // ⇧⌘↩ on t2: alone over the tab, the others kept in its tree as they were.
    desk.zoomed("t2", true).unwrap();
    assert!(zoomed_on(&desk, "t2"));
    assert_eq!(tab_of(&desk, "t1").tree.panes(), panes.panes());
    // The next pane shown takes the zoom; back out, the tab is drawn whole again.
    desk.focused("t3").unwrap();
    assert!(zoomed_on(&desk, "t3"));
    desk.zoomed("t3", false).unwrap();
    assert!(!tab_of(&desk, "t1").whole);

    // A divider moved ends it.
    desk.zoomed("t1", true).unwrap();
    desk.resized("t1", &[], 1, 6000).unwrap();
    assert!(!tab_of(&desk, "t1").whole);
    // So does the tab made even.
    desk.zoomed("t1", true).unwrap();
    desk.equalized("t1").unwrap();
    assert!(!tab_of(&desk, "t1").whole);
    // A new split ends it, the new pane in sight beside the others.
    desk.zoomed("t1", true).unwrap();
    let new = desk
        .split_new_told(&person(&mine), "t1", Side::Down, "sleep 30")
        .unwrap();
    let tab = tab_of(&desk, "t1");
    assert!(!tab.whole);
    assert_eq!(tab.tree.visible().len(), 4, "{new} with the three");
    // And a close: the zoomed pane's tab is drawn whole without it.
    desk.zoomed(&new, true).unwrap();
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t3"}))
        .unwrap();
    assert!(!tab_of(&desk, "t1").whole);
    desk.shutdown();
}

#[test]
fn a_pane_alone_in_its_tab_has_nothing_to_zoom() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    desk.window(&mine.call(), &serde_json::from_str(TERMINAL).unwrap())
        .unwrap();
    assert!(!desk.zoomed("t1", true).unwrap());
    assert!(!tab_of(&desk, "t1").whole);
    desk.shutdown();
}
