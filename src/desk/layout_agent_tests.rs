//! Tests of the layout as the window tool reaches it: `open` beside a pane of the project (a
//! terminal on a side of it, a document in its session), the person's focus left where it was;
//! what `beside` and `side` refuse; `show` with `zoom`; and `list` saying each pane's tab and
//! what lies beside it.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::desk::tests::{caller, desk_on};
use crate::layout;
use crate::testing::Scratch;

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

fn terminal(more: &Value) -> Value {
    let mut asked = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "yield_ms": 200});
    for (field, value) in more.as_object().into_iter().flatten() {
        asked[field] = value.clone();
    }
    asked
}

#[test]
fn an_agent_opens_a_terminal_and_a_file_beside_a_pane_and_the_focus_stays() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("a.rs", "fn main() {}\n");
    let mine = caller(&desk, scratch.path(), None);
    let call = |args: Value| desk.window(&mine.call(), &args).unwrap();
    call(terminal(&json!({})));
    let row = |key: &str| crate::lock(&desk.ledger)[key].0;
    let focused = tab_of(&desk, "t1").focused;

    // A terminal under t1: one tab of the two, t1's focus as it was.
    let opened = call(terminal(&json!({"beside": "t1", "side": "down"})));
    assert!(opened.contains("t2 (terminal)"), "{opened}");
    let tab = tab_of(&desk, "t1");
    assert_eq!(tab.tree.visible(), [row("t1"), row("t2")]);
    assert_eq!(tab.tree.path_of(&row("t2")), Some(vec![1]));
    assert!(matches!(
        tab.tree,
        layout::Tree::Branch { across: false, .. }
    ));
    assert_eq!(tab.focused, focused);
    // A file beside t2: a document of that session, on the right of its terminals.
    call(json!({"action": "open", "kind": "file", "path": "a.rs", "beside": "t2"}));
    let tab = tab_of(&desk, "t1");
    assert_eq!(tab.tree.stacked(), [row("f1")]);
    assert_eq!(tab.focused, focused);

    // `list`: each pane's tab, and what lies on each side of it.
    let listed = call(json!({"action": "list"}));
    let line = |key: &str| {
        (listed.lines())
            .find(|line| line.starts_with(&format!("{key} (")))
            .unwrap_or_default()
            .to_string()
    };
    assert!(
        line("t1").contains("in tab 1, f1 on its right, t2 below it:"),
        "{listed}"
    );
    assert!(
        line("t2").contains("in tab 1, f1 on its right, t1 above it:"),
        "{listed}"
    );
    assert!(
        line("f1").contains("in tab 1, t1 and t2 on its left:"),
        "{listed}"
    );
    desk.shutdown();
}

#[test]
fn beside_names_a_pane_of_the_project_and_side_goes_with_it() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    scratch.file("a.rs", "fn main() {}\n");
    let mine = caller(&desk, scratch.path(), None);
    let call = |args: Value| desk.window(&mine.call(), &args).unwrap();
    call(terminal(&json!({})));
    let refused = [
        (
            terminal(&json!({"beside": "t9"})),
            "t9 is no tab of this project",
        ),
        (
            terminal(&json!({"side": "down"})),
            "`side` goes with `beside`",
        ),
        (
            terminal(&json!({"beside": "t1", "side": "left"})),
            "`side` is right or down",
        ),
        (
            json!({"action": "open", "kind": "file", "path": "a.rs", "beside": "t1",
                "side": "down"}),
            "a document joins the documents of the tab beside",
        ),
        (
            json!({"action": "open", "kind": "board", "beside": "t1"}),
            "`beside` is for a terminal, a page or a file",
        ),
    ];
    for (asked, said) in refused {
        let report = call(asked);
        assert!(report.contains(said), "{report}");
    }
    // Nothing of them was opened: t1 is alone in its tab.
    assert_eq!(tab_of(&desk, "t1").tree.panes().len(), 1);
    // Halved and halved again, a pane keeps a tenth of its branch at the least: past it,
    // there is no room beside it.
    for _ in 0..3 {
        assert!(call(terminal(&json!({"beside": "t1"}))).contains("(terminal)"));
    }
    let crowded = call(terminal(&json!({"beside": "t1"})));
    assert!(crowded.contains("no room for another pane"), "{crowded}");
    desk.shutdown();
}

#[test]
fn show_with_zoom_puts_the_others_by_and_list_says_so() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(store), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let call = |args: Value| desk.window(&mine.call(), &args).unwrap();
    call(terminal(&json!({})));
    call(terminal(&json!({"beside": "t1"})));
    let said = desk.zoom_shown("t2", true).unwrap();
    assert_eq!(said, "t2 fills its tab; 1 pane put by.");
    let tab = tab_of(&desk, "t1");
    assert!(tab.whole);
    assert_eq!(tab.focused, Some(crate::lock(&desk.ledger)["t2"].0));
    let listed = call(json!({"action": "list"}));
    assert!(
        listed.contains("t1 (terminal) in tab 1, zoomed on t2, t2 on its right:"),
        "{listed}"
    );
    assert_eq!(
        desk.zoom_shown("t2", false).unwrap(),
        "t2's tab shows all its panes."
    );
    assert!(!tab_of(&desk, "t1").whole);
    // A terminal of its own tab has nothing to put by.
    call(terminal(&json!({})));
    assert_eq!(
        desk.zoom_shown("t3", true).unwrap(),
        "t3 is alone in its tab: nothing to zoom."
    );
    desk.shutdown();
}
