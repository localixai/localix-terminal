//! Tests of a split told of a session: a document dragged beside a terminal joins it, and a
//! session with documents takes no other pane.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{caller, desk_on};
use crate::browser::Halves;
use crate::layout;
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

#[test]
fn a_document_put_beside_a_terminal_joins_it_and_a_session_takes_no_other_pane() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("a.rs", "fn main() {}\n");
    let mine = caller(&desk, scratch.path(), None);
    let open = |args: serde_json::Value| desk.window(&mine.call(), &args).unwrap();
    open(serde_json::from_str(TERMINAL).unwrap());
    open(serde_json::from_str(TERMINAL).unwrap());
    open(json!({"action": "open", "kind": "file", "path": "a.rs"}));
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;
    let tree_of = |key: &str| {
        (desk.feed.store())
            .read(|db| {
                let tab = layout::tab_of(db, row(key))?.unwrap();
                Ok(layout::tab(db, tab)?.unwrap())
            })
            .unwrap()
    };

    // The person looks at the loose file and carries a terminal beside it, on its right: the
    // file is that terminal's document now, and still the pane in use.
    desk.split_told(Halves {
        shown: "f1",
        beside: "t1",
        right: true,
    });
    let tab = tree_of("t1");
    assert_eq!(tab.tree.visible(), [row("f1"), row("t1")]);
    assert_eq!(tab.tree.stacked(), [row("f1")]);
    assert_eq!(tab.focused, Some(row("f1")));

    // Another terminal carried beside that session is refused: the session is as it was, and
    // the terminal still a tab of its own.
    desk.split_told(Halves {
        shown: "t1",
        beside: "t2",
        right: true,
    });
    assert_eq!(tree_of("t1").tree, tab.tree);
    assert_eq!(tree_of("t2").tree, layout::Tree::leaf(row("t2")));
    desk.shutdown();
}
