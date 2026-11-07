//! A divider's final size is durable, and repeating it creates no extra change.

use std::sync::Arc;

use super::*;
use crate::browser::Halves;
use crate::desk::tests::{caller, desk_on};
use crate::store::Store;
use crate::testing::Scratch;

#[test]
fn a_split_keeps_its_ratio_and_a_repeated_or_invalid_size_changes_nothing() {
    let scratch = Scratch::new();
    let store = Arc::new(Store::memory().unwrap());
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let project = mine.project.id;
    for _ in 0..2 {
        desk.window(&mine.call(), &serde_json::json!({"action":"open", "kind":"terminal", "command":"sleep 10", "yield_ms":200}))
            .unwrap();
    }
    desk.split_told(
        &crate::projects::key(project),
        Some(Halves {
            shown: "t1",
            beside: "t2",
            right: true,
        }),
    );
    desk.split_resized("t1", 6500).unwrap();
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
    desk.split_resized("t2", 6500).unwrap();
    assert!(desk.split_resized("t1", 0).is_err());
    assert_eq!(latest(), before);
    desk.shutdown();
}
