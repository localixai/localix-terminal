//! Tests of a session and its documents: a file asked from a terminal joins that terminal's
//! tab, a stack on its right with one in sight; the person's joins the session they look at; and
//! what is asked from no session joins a session of its project all the same.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{Caller, caller, desk_on};
use super::super::{Call, Desk};
use crate::browser::Halves;
use crate::layout;
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

/// `mine`'s call, come from a program in terminal `from`.
fn from<'a>(mine: &'a Caller, from: &'a str) -> Call<'a> {
    Call {
        from: Some(from),
        ..mine.call()
    }
}

/// A terminal opened by `call`.
fn open(desk: &Desk, call: &Call) {
    desk.window(call, &serde_json::from_str(TERMINAL).unwrap())
        .unwrap();
}

/// The tabs of the first window's workspace of `mine`'s project, each as the keys of its panes
/// in sight (a tab whole: its pane in focus alone), with the pane in focus of each.
fn tabs(desk: &Desk, mine: &Caller) -> Vec<(Vec<String>, Option<String>)> {
    let keys = crate::lock(&desk.ledger).clone();
    let key = |row: i64| {
        (keys.iter())
            .find(|(_, (kept, _))| *kept == row)
            .map_or_else(|| format!("#{row}"), |(key, _)| key.clone())
    };
    let project = mine.project.id;
    (desk.feed.store())
        .read(|db| {
            let Some(window) = layout::windows(db)?.first().map(|window| window.id) else {
                return Ok(vec![]);
            };
            let space = (layout::workspaces(db, window)?.into_iter())
                .find(|space| space.project == project);
            let Some(space) = space else {
                return Ok(vec![]);
            };
            layout::tabs(db, space.id)
        })
        .unwrap()
        .into_iter()
        .map(|tab| {
            let sight = match (tab.whole, tab.focused) {
                (true, Some(focused)) => vec![focused],
                _ => tab.tree.visible(),
            };
            let panes = sight.into_iter().map(key).collect();
            (panes, tab.focused.map(key))
        })
        .collect()
}

/// The documents of each tab, in their order.
fn docs(desk: &Desk, mine: &Caller) -> Vec<Vec<String>> {
    let keys = crate::lock(&desk.ledger).clone();
    let key = |row: i64| {
        (keys.iter())
            .find(|(_, (kept, _))| *kept == row)
            .map_or_else(|| format!("#{row}"), |(key, _)| key.clone())
    };
    let project = mine.project.id;
    (desk.feed.store())
        .read(|db| {
            let window = layout::windows(db)?[0].id;
            let space = (layout::workspaces(db, window)?.into_iter())
                .find(|space| space.project == project);
            layout::tabs(db, space.map_or(0, |space| space.id))
        })
        .unwrap()
        .into_iter()
        .map(|tab| tab.tree.stacked().into_iter().map(key).collect())
        .collect()
}

/// The panes in sight of each tab, without their focus.
fn panes(desk: &Desk, mine: &Caller) -> Vec<Vec<String>> {
    (tabs(desk, mine).into_iter())
        .map(|(panes, _)| panes)
        .collect()
}

fn started() -> (Arc<Desk>, Scratch) {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    (desk_on(store), Scratch::new())
}

/// The window shows the tab of pane `key`, that pane in focus: what it tells as the person goes
/// to a tab.
fn sees(desk: &Desk, key: &str) {
    let row = crate::lock(&desk.ledger)[key].0;
    desk.show_pane(row).unwrap();
}

/// `mine`'s call as the person makes it in the window.
fn person(mine: &Caller) -> Call<'_> {
    Call {
        person: true,
        ..mine.call()
    }
}

/// File `name` of the scratch folder opened by `call`: its key.
fn file(desk: &Desk, call: &Call, name: &str) -> String {
    let said = desk
        .window(
            call,
            &json!({"action": "open", "kind": "file", "path": name}),
        )
        .unwrap();
    let key = said.split_whitespace().next().unwrap_or_default();
    key.trim_end_matches(':').to_string()
}

/// A scratch folder with files `a.rs`, `b.rs` and `c.rs`.
fn sources() -> (Arc<Desk>, Scratch) {
    let (desk, scratch) = started();
    for name in ["a.rs", "b.rs", "c.rs"] {
        scratch.file(name, "fn main() {}\n");
    }
    (desk, scratch)
}

#[test]
fn what_an_agent_opens_from_a_terminal_joins_its_session_on_the_right() {
    let (desk, scratch) = sources();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());

    // The agent in t1 opens a file: on the terminal's right, which stays the pane in use.
    file(&desk, &from(&mine, "t1"), "a.rs");
    assert_eq!(
        tabs(&desk, &mine),
        [(vec!["t1".to_string(), "f1".into()], Some("t1".into()))]
    );

    // The next one is the document in sight, and the one before waits behind it: one tab still.
    desk.split_resized("t1", 3000).unwrap();
    file(&desk, &from(&mine, "t1"), "b.rs");
    assert_eq!(panes(&desk, &mine), [["t1", "f2"]]);
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);
    let tree = (desk.feed.store())
        .read(|db| {
            let row = crate::lock(&desk.ledger)["t1"].0;
            let tab = layout::tab_of(db, row)?.unwrap();
            Ok(layout::tab(db, tab)?.unwrap().tree)
        })
        .unwrap();
    let layout::Tree::Branch { children, .. } = tree else {
        panic!("the tab is not split");
    };
    assert!((children[0].share - 0.3).abs() < 1e-9, "{children:?}");

    // A document of its shown from the terminal is the one in sight again.
    desk.window(&from(&mine, "t1"), &json!({"action": "show", "tab": "f1"}))
        .unwrap();
    assert_eq!(panes(&desk, &mine), [["t1", "f1"]]);
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);

    // A terminal it opens is a session of its own.
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [vec!["t1", "f1"], vec!["t2"]]);
    desk.shutdown();
}

#[test]
fn a_file_the_person_picks_joins_the_session_they_look_at_and_is_the_pane_in_use() {
    let (desk, scratch) = sources();
    let mine = caller(&desk, scratch.path(), None);
    let pick = |name: &str| {
        desk.person_file(&person(&mine), &scratch.path().join(name))
            .unwrap();
    };
    open(&desk, &mine.call());
    open(&desk, &mine.call());
    sees(&desk, "t1");

    pick("a.rs");
    pick("b.rs");
    assert_eq!(
        tabs(&desk, &mine),
        [
            (vec!["t1".to_string(), "f2".into()], Some("f2".into())),
            (vec!["t2".to_string()], Some("t2".into()))
        ]
    );

    // The person is in a document: what the agent opens waits behind it, a tab over the stack.
    file(&desk, &from(&mine, "t1"), "c.rs");
    assert_eq!(panes(&desk, &mine)[0], ["t1", "f2"]);
    assert_eq!(docs(&desk, &mine)[0], ["f1", "f2", "f3"]);

    // Each session keeps its own: a file open in one, picked in the other, moves there.
    sees(&desk, "t2");
    pick("a.rs");
    assert_eq!(docs(&desk, &mine), [vec!["f2", "f3"], vec!["f1"]]);
    assert_eq!(panes(&desk, &mine), [["t1", "f2"], ["t2", "f1"]]);
    desk.shutdown();
}

#[test]
fn what_is_asked_from_no_session_joins_a_session_of_its_project() {
    let (desk, scratch) = sources();
    scratch.file("e.rs", "fn main() {}\n");
    let other = Scratch::new();
    other.file("d.rs", "fn main() {}\n");
    let (mine, theirs) = (
        caller(&desk, scratch.path(), None),
        caller(&desk, other.path(), None),
    );

    // A project with no terminal yet: its files are one tab, the last opened in sight.
    file(&desk, &mine.call(), "a.rs");
    file(&desk, &mine.call(), "b.rs");
    assert_eq!(panes(&desk, &mine), [["f2"]]);
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);

    // Its first terminal is a session, and takes them as they were: no tab is left that is
    // not a session.
    open(&desk, &mine.call());
    assert_eq!(panes(&desk, &mine), [["t1", "f2"]]);
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);

    // A second session: an agent with no terminal here opens in the session shown, and where
    // the project shows none of them, in its last.
    open(&desk, &theirs.call());
    open(&desk, &mine.call());
    sees(&desk, "t1");
    file(&desk, &mine.call(), "c.rs");
    assert_eq!(docs(&desk, &mine), [vec!["f1", "f2", "f3"], vec![]]);
    sees(&desk, "t3");
    file(&desk, &mine.call(), "e.rs");
    assert_eq!(docs(&desk, &mine), [vec!["f1", "f2", "f3"], vec!["f4"]]);
    assert_eq!(panes(&desk, &mine), [["t1", "f3"], ["t3", "f4"]]);

    // A terminal of another project is no session of this one's, nor its file this one's.
    let astray = Call {
        from: Some("t2"),
        ..mine.call()
    };
    file(&desk, &astray, "a.rs");
    assert_eq!(panes(&desk, &theirs), [["t2"]]);
    assert_eq!(docs(&desk, &mine), [vec!["f1", "f2", "f3"], vec!["f4"]]);
    let not = desk.person_file(&person(&mine), &other.path().join("d.rs"));
    assert!(not.is_err(), "{not:?}");
    desk.shutdown();
}

#[test]
fn a_session_made_whole_keeps_its_documents_until_it_is_split_again() {
    let (desk, scratch) = sources();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    file(&desk, &from(&mine, "t1"), "a.rs");
    let halves = Halves {
        shown: "t1",
        beside: "f1",
        right: true,
    };

    // The person makes the tab whole: the terminal alone, its document still its own.
    desk.whole_told(halves);
    assert_eq!(panes(&desk, &mine), [["t1"]]);
    assert_eq!(docs(&desk, &mine), [["f1"]]);

    // What the agent opens joins the documents put by, and the tab stays whole.
    file(&desk, &from(&mine, "t1"), "b.rs");
    assert_eq!(panes(&desk, &mine), [["t1"]]);
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);

    // The person puts a document beside the terminal again: that one is in sight.
    desk.split_told(halves);
    assert_eq!(
        tabs(&desk, &mine),
        [(vec!["t1".to_string(), "f1".into()], Some("t1".into()))]
    );

    // A document closed by the window tells its split whole too, and that is no one's wish.
    desk.window(&mine.call(), &json!({"action": "close", "tab": "f1"}))
        .unwrap();
    desk.whole_told(halves);
    assert_eq!(panes(&desk, &mine), [["t1", "f2"]]);
    desk.shutdown();
}

#[test]
fn a_document_shown_by_the_person_is_the_one_in_sight_and_the_halves_change_sides() {
    let (desk, scratch) = sources();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    file(&desk, &from(&mine, "t1"), "a.rs");
    file(&desk, &from(&mine, "t1"), "b.rs");

    // The person picks the tab of f1 over the stack: it is in sight and in use.
    desk.split_told(Halves {
        shown: "f1",
        beside: "t1",
        right: false,
    });
    assert_eq!(
        tabs(&desk, &mine),
        [(vec!["t1".to_string(), "f1".into()], Some("f1".into()))]
    );

    // They carry the terminal to the right: the documents are on the left from now.
    desk.split_told(Halves {
        shown: "t1",
        beside: "f1",
        right: false,
    });
    assert_eq!(
        tabs(&desk, &mine),
        [(vec!["f1".to_string(), "t1".into()], Some("t1".into()))]
    );
    assert_eq!(docs(&desk, &mine), [["f1", "f2"]]);
    desk.shutdown();
}

#[test]
fn a_session_closed_closes_its_documents_with_it() {
    let (desk, scratch) = sources();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    open(&desk, &mine.call());
    file(&desk, &from(&mine, "t1"), "a.rs");
    file(&desk, &from(&mine, "t1"), "b.rs");
    file(&desk, &from(&mine, "t2"), "c.rs");
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    // Its documents close on a thread of their own.
    let left = || panes(&desk, &mine);
    crate::harness::Home::until("the session's documents close", || left().len() == 1);
    assert_eq!(left(), [["t2", "f3"]]);
    // A call that still names the closed terminal (a program that outlived it) has no session
    // of its own: its file joins the one the project has.
    file(&desk, &from(&mine, "t1"), "a.rs");
    assert_eq!(docs(&desk, &mine), [["f3", "f4"]]);
    desk.shutdown();
}

#[test]
fn a_call_is_known_by_the_terminal_its_program_descends_from() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    // A shell in the terminal starts a program and prints its pid: what an agent's tool is.
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'sleep 30 & echo pid=$!; echo told; wait'", "wait_for": "told"});
    let text = desk.window(&mine.call(), &open).unwrap();
    let pid = text.split("pid=").nth(1).unwrap();
    let digits: String = pid.chars().take_while(char::is_ascii_digit).collect();
    let pid: libc::pid_t = digits.parse().unwrap();
    assert_eq!(desk.pane_of(pid).as_deref(), Some("t1"));
    // This test is no terminal's program.
    let own = libc::pid_t::try_from(std::process::id()).unwrap();
    assert_eq!(desk.pane_of(own), None);
    desk.shutdown();
}
