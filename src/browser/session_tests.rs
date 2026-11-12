//! Tests of a session as the window reads it from the layout: the row that stands for it, the
//! split a tab picked is laid out in, and what may be put beside what.

use super::*;
use crate::browser::{Arranged, Arranging};

fn keys(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

/// A session of terminal `t1` with files `f1` and `f2` (`f2` in sight, on the right), a lone
/// terminal `t2`, the loose documents `w3` and `f4` (`f4` in sight), and a board.
fn arranged() -> Arranged {
    let tab = |panes: &[&str], docs: &[&str], all: &[&str]| Arranging {
        panes: keys(panes),
        across: true,
        shares: vec![0.5; panes.len()],
        focused: panes.last().map(ToString::to_string),
        docs: keys(docs),
        all: keys(all),
        whole: false,
    };
    Arranged {
        window: 1,
        group: "a".into(),
        shown: Some("t1".into()),
        tabs: vec![
            tab(&["t1", "f2"], &["f1", "f2"], &["t1", "f1", "f2"]),
            tab(&["t2"], &[], &["t2"]),
            tab(&["f4"], &["w3", "f4"], &["w3", "f4"]),
            tab(&["b1"], &[], &["b1"]),
        ],
    }
}

fn file(id: u32) -> Key {
    Key::parse(&format!("f{id}")).unwrap()
}

#[test]
fn a_session_is_listed_by_its_terminal_and_loose_documents_by_the_one_in_sight() {
    let led = led(&arranged(), None);
    let of = |key: &str| led.get(key).map(String::as_str);
    assert_eq!(of("f1"), Some("t1"));
    assert_eq!(of("f2"), Some("t1"));
    assert_eq!(of("w3"), Some("f4"));
    // What stands for its tab is listed itself.
    for own in ["t1", "t2", "f4", "b1"] {
        assert_eq!(of(own), None, "{own}");
    }
    // The window shows a loose document the layout has not heard of yet: that one is listed.
    let led = super::led(&arranged(), Some("w3"));
    assert_eq!(led.get("f4").map(String::as_str), Some("w3"));
    assert_eq!(led.get("w3"), None);
}

#[test]
fn a_document_the_layout_has_not_told_of_is_no_row_of_the_side() {
    let arranged = arranged();
    let of = |key: &str, shown: Option<&str>| stands_for(Some(&arranged), key, shown);
    // A page just opened, the session it opens in in sight: that session stands for it, by
    // its terminal whichever of its panes the window shows.
    assert_eq!(of("w9", Some("t1")).as_deref(), Some("t1"));
    assert_eq!(of("f9", Some("f1")).as_deref(), Some("t1"));
    // Shown itself, or with nothing in sight: no row is lit, and it is still none itself.
    assert_eq!(of("w9", Some("w9")).as_deref(), Some("w9"));
    assert_eq!(of("w9", None).as_deref(), Some("w9"));
    // A project the layout says nothing of has no session to wait for: its tab is listed as
    // it comes (a window with no layout lists every tab).
    assert_eq!(stands_for(None, "w9", None), None);
    // What the layout holds is listed as it says, and a terminal or the board always is.
    for placed in ["f1", "f4", "w3", "t1", "t9", "b9"] {
        assert_eq!(of(placed, Some("t1")), None, "{placed}");
    }
}

#[test]
fn a_tab_picked_is_split_as_its_session_is_at_once() {
    let arranged = arranged();
    let kept = |shown: &str, beside: &str, right: bool| {
        Some(Kept {
            shown: shown.into(),
            beside: beside.into(),
            right,
        })
    };
    assert_eq!(split_of(&arranged, "t1"), kept("t1", "f2", true));
    assert_eq!(split_of(&arranged, "f2"), kept("f2", "t1", false));
    // A document put by takes the place of the one in sight.
    assert_eq!(split_of(&arranged, "f1"), kept("f1", "t1", false));
    // A lone terminal, the loose documents and a tab the layout does not hold are whole.
    for whole in ["t2", "f4", "w3", "b1", "t9"] {
        assert_eq!(split_of(&arranged, whole), None, "{whole}");
    }
    // Kept whole by the person, a session shows its pane alone.
    let mut put_by = arranged.clone();
    put_by.tabs[0].whole = true;
    assert_eq!(split_of(&put_by, "t1"), None);
    assert_eq!(beside_whole(&put_by, "t1"), kept("t1", "f2", true));
    assert_eq!(beside_whole(&arranged, "t1"), None);
}

#[test]
fn a_session_takes_its_own_documents_and_loose_ones_and_no_other_pane() {
    let arranged = arranged();
    let takes = |shown: &str, beside: &str| {
        takes(
            &arranged,
            Key::parse(shown).unwrap(),
            Key::parse(beside).unwrap(),
        )
    };
    assert!(takes("t1", "f2"));
    assert!(takes("f1", "t1"));
    // Two documents of one stack are never side by side.
    assert!(!takes("f1", "f2"));
    assert!(!takes("w3", "f4"));
    // A loose document joins a session, or a terminal alone.
    assert!(takes("t1", "f4"));
    assert!(takes("w3", "t2"));
    // A session takes no other terminal and no board; terminals alone pair as before.
    assert!(!takes("t1", "t2"));
    assert!(!takes("b1", "f1"));
    assert!(!takes("b1", "f4"));
    assert!(takes("t2", "b1"));
}

#[test]
fn the_documents_row_lies_over_the_one_in_sight() {
    let stacks = stacks([&arranged()]);
    assert_eq!(stacks, [vec![file(1), file(2)], vec![Key::Web(3), file(4)]]);
    let term = Key::Term(1);
    // A split session: over its document's half, whichever is in use.
    let row = row(&stacks, Some(term), Some((file(2), term)));
    assert_eq!(row, Some((file(2), &stacks[0])));
    let row = super::row(&stacks, Some(file(1)), Some((file(1), term)));
    assert_eq!(row.map(|row| row.0), Some(file(1)));
    // Whole: over the document shown; a terminal alone has none.
    assert_eq!(
        super::row(&stacks, Some(Key::Web(3)), None).map(|row| row.0),
        Some(Key::Web(3))
    );
    assert_eq!(super::row(&stacks, Some(term), None), None);
    // The one next to a document closed: the one after it, else the one before.
    assert_eq!(next(&stacks, file(1)), Some(file(2)));
    assert_eq!(next(&stacks, file(4)), Some(Key::Web(3)));
    assert_eq!(next(&stacks, term), None);
}
