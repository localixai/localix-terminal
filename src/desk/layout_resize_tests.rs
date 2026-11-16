//! A divider's final size is durable, and repeating it creates no extra change; a tab of three
//! panes is resized by any of its dividers, and the pane the person puts the keys in is the
//! one in focus.

use std::sync::Arc;

use super::*;
use crate::browser::Side;
use crate::desk::tests::{caller, desk_on};
use crate::layout::Tree;
use crate::store::Store;
use crate::testing::Scratch;

#[test]
fn a_split_keeps_its_ratio_and_a_repeated_or_invalid_size_changes_nothing() {
    let scratch = Scratch::new();
    let store = Arc::new(Store::memory().unwrap());
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    for _ in 0..2 {
        desk.window(&mine.call(), &serde_json::json!({"action":"open", "kind":"terminal", "command":"sleep 10", "yield_ms":200}))
            .unwrap();
    }
    desk.split_told("t2", "t1", Side::Right);
    desk.resized("t1", &[], 1, 6500).unwrap();
    let pane = crate::lock(&desk.ledger)["t1"].0;
    let sized = store
        .read(|db| {
            let id = layout::tab_of(db, pane)?.unwrap();
            Ok(layout::tab(db, id)?.unwrap())
        })
        .unwrap();
    let Tree::Branch { children, .. } = sized.tree else {
        panic!("not a split")
    };
    assert_eq!(
        children.iter().map(|one| one.share).collect::<Vec<_>>(),
        [0.65, 0.35]
    );
    let latest = || {
        store
            .read(|db| {
                db.query_row(
                    "SELECT COALESCE(MAX(seq),0) FROM change_events",
                    [],
                    |row| row.get::<_, u64>(0),
                )
                .map_err(|why| why.to_string())
            })
            .unwrap()
    };
    let before = latest();
    desk.resized("t2", &[], 1, 6500).unwrap();
    assert!(desk.resized("t1", &[], 1, 0).is_err());
    assert!(desk.resized("t1", &[], 1, 10_000).is_err());
    // A branch the tree does not have, or a divider past its children, changes nothing.
    desk.resized("t1", &[0], 1, 5000).unwrap();
    desk.resized("t1", &[], 2, 5000).unwrap();
    assert_eq!(latest(), before);
    desk.shutdown();
}

/// The tree of the tab pane `key` is in, and the pane in focus.
fn tab_of(desk: &Desk, store: &Store, key: &str) -> (Tree, Option<Row>) {
    let pane = crate::lock(&desk.ledger)[key].0;
    let tab = store
        .read(|db| {
            let id = layout::tab_of(db, pane)?.unwrap();
            Ok(layout::tab(db, id)?.unwrap())
        })
        .unwrap();
    (tab.tree, tab.focused)
}

#[test]
fn three_panes_split_by_the_desk_are_resized_by_either_divider_and_focused_by_the_person() {
    let scratch = Scratch::new();
    let store = Arc::new(Store::memory().unwrap());
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    for _ in 0..3 {
        desk.window(&mine.call(), &serde_json::json!({"action":"open", "kind":"terminal", "command":"sleep 10", "yield_ms":200}))
            .unwrap();
    }
    // A pane put beside one of a split of the same way is one more child of its branch.
    desk.split_told("t2", "t1", Side::Right);
    desk.split_told("t3", "t1", Side::Right);
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;
    let (tree, _) = tab_of(&desk, &store, "t2");
    assert_eq!(tree.panes(), [row("t1"), row("t3"), row("t2")]);
    let shares = |tree: &Tree| match tree {
        Tree::Branch { children, .. } => children.iter().map(|one| one.share).collect(),
        Tree::Leaf { .. } | Tree::Stack { .. } => vec![],
    };
    assert_eq!(shares(&tree), [0.25, 0.25, 0.5]);
    // The second divider: the first two take three quarters, in their proportions.
    desk.resized("t3", &[], 2, 7500).unwrap();
    let (tree, _) = tab_of(&desk, &store, "t1");
    let near =
        |one: Vec<f64>, other: [f64; 3]| one.iter().zip(other).all(|(a, b)| (a - b).abs() < 1e-9);
    assert!(
        near(shares(&tree), [0.375, 0.375, 0.25]),
        "{:?}",
        shares(&tree)
    );
    // The person puts the keys in t2: the tab's pane in focus.
    desk.focused("t2").unwrap();
    assert_eq!(tab_of(&desk, &store, "t1").1, Some(row("t2")));
    // Made whole from t3, t3 is the pane that stays, whichever was in focus.
    desk.whole_told("t3");
    assert_eq!(tab_of(&desk, &store, "t3").0, Tree::leaf(row("t3")));
    desk.shutdown();
}
