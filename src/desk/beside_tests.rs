//! Tests of what opens beside the terminal it was asked from: an agent's page or terminal lands
//! on the right of the terminal the agent runs in, the next takes its place, and the person's
//! own arrangement is never undone.

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

/// The tabs of the first window's workspace of `mine`'s project, each as its panes' keys, with
/// the pane in focus of each.
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
            let panes = tab.tree.panes().into_iter().map(key).collect();
            (panes, tab.focused.map(key))
        })
        .collect()
}

/// The panes of each tab, without their focus.
fn panes(desk: &Desk, mine: &Caller) -> Vec<Vec<String>> {
    (tabs(desk, mine).into_iter())
        .map(|(panes, _)| panes)
        .collect()
}

fn started() -> (Arc<Desk>, Scratch) {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    (desk_on(store), Scratch::new())
}

#[test]
fn what_is_opened_from_a_terminal_lands_on_its_right_and_the_next_takes_its_place() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    assert_eq!(panes(&desk, &mine), [["t1"]]);

    // The agent in t1 opens a terminal: beside t1, which stays the pane in use.
    open(&desk, &from(&mine, "t1"));
    assert_eq!(
        tabs(&desk, &mine),
        [(vec!["t1".to_string(), "t2".into()], Some("t1".into()))]
    );

    // The person drags the line; the next one the agent opens takes the place and its width,
    // and the one before is a tab of its own, right after.
    desk.split_resized("t1", 7000).unwrap();
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [vec!["t1", "t3"], vec!["t2"]]);
    let shares = (desk.feed.store())
        .read(|db| {
            let row = crate::lock(&desk.ledger)["t1"].0;
            let tab = layout::tab_of(db, row)?.unwrap();
            Ok(layout::tab(db, tab)?.unwrap().tree)
        })
        .unwrap();
    let layout::Tree::Branch { children, .. } = shares else {
        panic!("the tab is not split");
    };
    assert!((children[0].share - 0.7).abs() < 1e-9, "{children:?}");

    // A tab of its shown from the terminal comes back beside it.
    desk.window(&from(&mine, "t1"), &json!({"action": "show", "tab": "t2"}))
        .unwrap();
    assert_eq!(panes(&desk, &mine), [vec!["t1", "t2"], vec!["t3"]]);
    desk.shutdown();
}

#[test]
fn a_split_the_person_made_whole_stays_whole_until_they_split_it_again() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [["t1", "t2"]]);

    // The person makes the tab whole: what the agent opens next is a tab of its own.
    desk.whole_told(Halves {
        shown: "t1",
        beside: "t2",
        right: true,
    });
    assert_eq!(panes(&desk, &mine), [["t1"], ["t2"]]);
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [["t1"], ["t2"], ["t3"]]);

    // They put one of the agent's beside the terminal again: the place is the agent's again.
    let halves = Halves {
        shown: "t1",
        beside: "t3",
        right: true,
    };
    desk.split_told(halves);
    open(&desk, &from(&mine, "t1"));
    assert_eq!(
        panes(&desk, &mine),
        [vec!["t1", "t4"], vec!["t3"], vec!["t2"]]
    );
    desk.shutdown();
}

#[test]
fn a_tab_the_person_put_beside_a_terminal_is_not_replaced() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    open(&desk, &mine.call());
    let halves = Halves {
        shown: "t1",
        beside: "t2",
        right: true,
    };
    desk.split_told(halves);
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [vec!["t1", "t2"], vec!["t3"]]);
    desk.shutdown();
}

#[test]
fn each_terminal_has_its_own_place_and_one_made_whole_leaves_the_other_split() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    open(&desk, &mine.call());
    open(&desk, &from(&mine, "t1"));
    open(&desk, &from(&mine, "t2"));
    assert_eq!(panes(&desk, &mine), [["t1", "t3"], ["t2", "t4"]]);

    // The agent in t2 closes its pane: t2's tab is whole, and that is no one's wish, so the
    // next it opens lands beside it again. t1's split is as it was.
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t4"}))
        .unwrap();
    desk.whole_told(Halves {
        shown: "t2",
        beside: "t4",
        right: true,
    });
    assert_eq!(panes(&desk, &mine), [vec!["t1", "t3"], vec!["t2"]]);
    open(&desk, &from(&mine, "t2"));
    assert_eq!(panes(&desk, &mine), [["t1", "t3"], ["t2", "t5"]]);

    // The person makes t1's whole: t2's stays.
    desk.whole_told(Halves {
        shown: "t1",
        beside: "t3",
        right: true,
    });
    assert_eq!(
        panes(&desk, &mine),
        [vec!["t1"], vec!["t3"], vec!["t2", "t5"]]
    );
    desk.shutdown();
}

#[test]
fn a_named_agent_and_a_caller_of_another_project_open_tabs_of_their_own() {
    let (desk, scratch, other) = {
        let (desk, scratch) = started();
        (desk, scratch, Scratch::new())
    };
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());

    // A worker the agent started runs in the same terminal: several of them would take the one
    // place in turn, so each opens a tab.
    let worker = caller(&desk, scratch.path(), Some("worker"));
    open(&desk, &from(&worker, "t1"));
    assert_eq!(panes(&desk, &mine), [["t1"], ["t2"]]);

    // A terminal of this project is no place for another project's pane.
    let theirs = caller(&desk, other.path(), None);
    open(&desk, &from(&theirs, "t1"));
    assert_eq!(panes(&desk, &mine), [["t1"], ["t2"]]);
    assert_eq!(panes(&desk, &theirs), [["t3"]]);
    desk.shutdown();
}

#[test]
fn a_terminal_closed_leaves_what_was_beside_it_a_tab_and_is_no_place_any_more() {
    let (desk, scratch) = started();
    let mine = caller(&desk, scratch.path(), None);
    open(&desk, &mine.call());
    open(&desk, &from(&mine, "t1"));
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert_eq!(panes(&desk, &mine), [["t2"]]);
    // A call that still names it (a program that outlived it) opens a tab like any other.
    open(&desk, &from(&mine, "t1"));
    assert_eq!(panes(&desk, &mine), [["t2"], ["t3"]]);
    assert!(crate::lock(&desk.links).is_empty());
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
