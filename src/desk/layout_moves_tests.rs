//! Tests of a document carried along its session's row: the stack keeps the order, the one in
//! sight stays in sight, and a move that changes nothing tells nothing.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{caller, desk_on};
use crate::layout;
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

#[test]
fn a_document_carried_along_its_row_is_kept_there_by_its_session() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(Arc::clone(&store)), Scratch::new());
    for name in ["a.rs", "b.rs", "c.rs"] {
        scratch.file(name, "fn main() {}\n");
    }
    let mine = caller(&desk, scratch.path(), None);
    let open = |args: serde_json::Value| desk.window(&mine.call(), &args).unwrap();
    open(serde_json::from_str(TERMINAL).unwrap());
    for name in ["a.rs", "b.rs", "c.rs"] {
        open(json!({"action": "open", "kind": "file", "path": name}));
    }
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;
    let session = || {
        (desk.feed.store())
            .read(|db| {
                let tab = layout::tab_of(db, row("t1"))?.unwrap();
                Ok(layout::tab(db, tab)?.unwrap())
            })
            .unwrap()
    };
    let told = || {
        store
            .read(|db| {
                db.query_row("SELECT count(*) FROM change_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|why| why.to_string())
            })
            .unwrap()
    };
    let (a, b, c) = (row("f1"), row("f2"), row("f3"));
    assert_eq!(session().tree.stacked(), [a, b, c]);
    let sight = session().tree.doc();

    desk.placed("f3", Some("f1"));
    assert_eq!(session().tree.stacked(), [c, a, b]);
    desk.placed("f3", None);
    assert_eq!(session().tree.stacked(), [a, b, c]);
    assert_eq!(session().tree.doc(), sight);

    // Where it is already, or before the terminal: nothing moves and nothing is told.
    let before = told();
    desk.placed("f1", Some("f2"));
    desk.placed("f2", Some("t1"));
    desk.placed("t1", Some("f1"));
    assert_eq!(session().tree.stacked(), [a, b, c]);
    assert_eq!(told(), before);
    desk.shutdown();
}
