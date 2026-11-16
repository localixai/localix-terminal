//! Tests of a tab's tree in the window: what is drawn of the tab shown, and what the person did
//! to it that waits for the layout (a split made or undone, a divider held, the focus).

use super::*;
use crate::browser::{Arranged, Arranging};
use crate::layout::{Share, Tree};

fn branch(across: bool, children: Vec<(f64, Drawn)>) -> Drawn {
    let children = (children.into_iter())
        .map(|(share, tree)| Share { share, tree })
        .collect();
    Tree::Branch { across, children }
}

fn leaf(key: &str) -> Drawn {
    Tree::leaf(key.to_string())
}

fn keys(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

/// A tab of `tree`, `focused` in focus, with no documents.
fn tab(tree: Drawn, focused: &str) -> Arranging {
    Arranging {
        all: tree.panes(),
        tree,
        focused: Some(focused.to_string()),
        docs: vec![],
        whole: false,
    }
}

/// `t1` beside `t2`, shares `first` and the rest.
fn pair(first: f64) -> Arranging {
    tab(
        branch(true, vec![(first, leaf("t1")), (1.0 - first, leaf("t2"))]),
        "t1",
    )
}

/// `t1` on the left; `t2` over `t3` on the right.
fn tee() -> Arranging {
    let right = branch(false, vec![(0.5, leaf("t2")), (0.5, leaf("t3"))]);
    tab(branch(true, vec![(0.5, leaf("t1")), (0.5, right)]), "t1")
}

fn first_share(tree: Option<Drawn>) -> Option<f64> {
    share_at(&tree?, &[], 1)
}

#[test]
fn the_tab_of_the_pane_shown_is_drawn_whole_tree_and_one_pane_alone_is_not_split() {
    let split = Split::default();
    let tee = tee();
    for shown in ["t1", "t2", "t3"] {
        assert_eq!(
            split.tree(Some(&tee), shown),
            Some(tee.tree.clone()),
            "{shown}"
        );
    }
    assert_eq!(split.tree(Some(&tab(leaf("t4"), "t4")), "t4"), None);
    assert_eq!(
        split.tree(None, "t4"),
        None,
        "a pane the layout holds in no tab yet"
    );
    // Kept whole, the tab draws its pane in focus alone.
    let mut whole = tee;
    whole.whole = true;
    assert_eq!(split.tree(Some(&whole), "t1"), None);
    assert_eq!(drawn(&whole), keys(&["t1"]));
}

#[test]
fn a_document_shown_takes_the_place_of_the_one_its_stack_has_in_sight() {
    let mut tree = leaf("t1");
    tree.stack("f1".to_string());
    tree.stack("f2".to_string());
    let session = Arranging {
        docs: keys(&["f1", "f2"]),
        ..tab(tree, "t1")
    };
    let sight = |shown| {
        Split::default()
            .tree(Some(&session), shown)
            .map(|tree| tree.visible())
    };
    assert_eq!(sight("t1"), Some(keys(&["t1", "f2"])));
    assert_eq!(sight("f1"), Some(keys(&["t1", "f1"])));
}

#[test]
fn a_split_the_person_made_is_drawn_until_the_layout_draws_it_or_moves_on() {
    // Split from t1 with no layout of it yet: drawn at once.
    let made = branch(true, vec![(0.5, leaf("t1")), (0.5, leaf("t2"))]);
    let mut split = Split {
        own: Some(Own {
            tree: made.clone(),
            base: keys(&["t1"]),
        }),
        ..Split::default()
    };
    let alone = tab(leaf("t1"), "t1");
    assert_eq!(split.tree(Some(&alone), "t1"), Some(made.clone()));
    assert_eq!(split.tree(None, "t1"), Some(made.clone()));
    // A tell as old as when they made it keeps it.
    split.follow(Some(&alone), "t1");
    assert!(split.own.is_some());
    // The layout draws it so: it is the layout's again, its own shares.
    split.follow(Some(&pair(0.35)), "t1");
    assert_eq!(split.own, None);
    assert_eq!(first_share(split.tree(Some(&pair(0.35)), "t1")), Some(0.35));
    // The layout drew another change of the tab first: the person's goes, the layout's shows.
    let mut split = Split {
        own: Some(Own {
            tree: made,
            base: keys(&["t1"]),
        }),
        ..Split::default()
    };
    split.follow(Some(&tee()), "t1");
    assert_eq!(split.own, None);
}

#[test]
fn a_tab_made_whole_draws_the_pane_in_use_alone_and_puts_the_others_out_of_it() {
    let tee = tee();
    let mut split = Split {
        own: Some(Own {
            tree: leaf("t2"),
            base: keys(&["t1", "t2", "t3"]),
        }),
        ..Split::default()
    };
    assert_eq!(split.tree(Some(&tee), "t2"), None);
    assert_eq!(
        split.tree(Some(&tee), "t3"),
        None,
        "an older tab drawn alone, as it will be"
    );
    // The side lists no split for it meanwhile.
    let arranged = Arranged {
        window: 0,
        group: "a".into(),
        tabs: vec![tee],
        shown: Some("t2".into()),
    };
    assert_eq!(listed(&arranged, split.own.as_ref()), None);
    assert_eq!(listed(&arranged, None), Some(keys(&["t1", "t2", "t3"])));
    split.follow(Some(&tab(leaf("t2"), "t2")), "t2");
    assert_eq!(split.own, None);
}

#[test]
fn a_divider_held_keeps_its_share_over_an_older_tell_until_the_layout_keeps_it() {
    let held = |share, grabbed| Held {
        panes: keys(&["t1", "t2"]),
        path: vec![],
        at: 1,
        share,
        grabbed,
    };
    let mut split = Split {
        held: vec![held(0.6, Some(0.5))],
        ..Split::default()
    };
    assert_eq!(first_share(split.tree(Some(&pair(0.5)), "t1")), Some(0.6));
    // Still held: no tell moves it.
    split.follow(Some(&pair(0.6)), "t1");
    assert!(!split.held.is_empty());
    // Let go: an older share is not drawn; the share the layout kept ends the hold.
    split.held = vec![held(0.6, None)];
    split.follow(Some(&pair(0.35)), "t1");
    assert_eq!(first_share(split.tree(Some(&pair(0.35)), "t1")), Some(0.6));
    split.follow(Some(&pair(0.6)), "t1");
    assert!(split.held.is_empty());
    // Another tab is not held to it.
    split.held = vec![held(0.6, None)];
    assert_eq!(first_share(split.tree(Some(&tee()), "t1")), Some(0.5));
    split.follow(Some(&tee()), "t1");
    assert!(split.held.is_empty());
}

#[test]
fn two_dividers_let_go_of_are_each_held_until_the_layout_keeps_each() {
    let tee = tee();
    let column = Held {
        panes: keys(&["t1", "t2", "t3"]),
        path: vec![1],
        at: 1,
        share: 0.7,
        grabbed: None,
    };
    let root = Held {
        path: vec![],
        share: 0.4,
        ..column.clone()
    };
    let mut split = Split {
        held: vec![column, root],
        ..Split::default()
    };
    let drawn = split.tree(Some(&tee), "t1").unwrap();
    assert_eq!(share_at(&drawn, &[], 1), Some(0.4));
    assert_eq!(share_at(&drawn, &[1], 1), Some(0.7));
    // The layout keeps the root's: the column's is still held.
    let mut kept = tee;
    kept.tree.resize(&[], 1, 0.4);
    split.follow(Some(&kept), "t1");
    assert_eq!(split.held.len(), 1);
    assert_eq!(split.held[0].path, [1]);
}

#[test]
fn the_focus_told_waits_until_the_layout_keeps_it() {
    let mut split = Split {
        focus: Some("t2".into()),
        ..Split::default()
    };
    split.follow(Some(&pair(0.5)), "t2");
    assert!(split.focus.is_some());
    split.follow(Some(&tab(pair(0.5).tree, "t2")), "t2");
    assert_eq!(split.focus, None);
}

#[test]
fn the_side_lists_the_shown_split_else_the_first_and_never_a_session() {
    let mut session_tree = leaf("t9");
    session_tree.stack("f9".to_string());
    let session = Arranging {
        docs: keys(&["f9"]),
        ..tab(session_tree, "t9")
    };
    let arranged = Arranged {
        window: 0,
        group: "a".into(),
        tabs: vec![session, pair(0.5), tee()],
        shown: Some("t3".into()),
    };
    assert_eq!(listed(&arranged, None), Some(keys(&["t1", "t2", "t3"])));
    let elsewhere = Arranged {
        shown: Some("t9".into()),
        ..arranged
    };
    assert_eq!(listed(&elsewhere, None), Some(keys(&["t1", "t2"])));
}

#[test]
fn the_partner_is_the_next_tab_else_the_one_before() {
    assert_eq!(partner(&[1, 2, 3], 2), Some(3));
    assert_eq!(partner(&[1, 2, 3], 3), Some(2));
    assert_eq!(partner(&[1], 1), None);
    assert_eq!(partner(&[1, 2], 9), None);
}

#[test]
fn a_divider_keeps_each_side_its_least_points() {
    let Tree::Branch { children, .. } = pair(0.5).tree else {
        panic!("not a branch")
    };
    let (low, high) = bounds(&children, true, 1, 1000.0);
    assert!((low - 0.19).abs() < 1e-9 && (high - 0.81).abs() < 1e-9);
    // A tenth at the least however wide; too narrow for both, half each.
    assert_eq!(bounds(&children, true, 1, 10_000.0), (0.1, 0.9));
    assert_eq!(bounds(&children, true, 1, 300.0), (0.5, 0.5));
    // Beside a column of two one above the other, the column keeps one pane's least across.
    let Tree::Branch { children, .. } = tee().tree else {
        panic!("not a branch")
    };
    let (low, high) = bounds(&children, true, 1, 1000.0);
    assert!((low - 0.19).abs() < 1e-9 && (high - 0.81).abs() < 1e-9);
}

#[test]
fn a_split_stays_under_a_list_open_over_it_and_stands_aside_where_nothing_lies_over_pages() {
    assert_eq!(under(Some(1), true, true), Some(1));
    assert_eq!(under(Some(1), true, false), None);
    assert_eq!(under(Some(1), false, false), Some(1));
}

#[test]
fn the_next_pane_goes_round_and_a_pane_alone_has_none() {
    let sight = keys(&["t1", "t2", "t3"]);
    let at = |shown: &str, forward| round(&sight, &shown.to_string(), forward);
    assert_eq!(at("t1", true).as_deref(), Some("t2"));
    assert_eq!(at("t3", true).as_deref(), Some("t1"));
    assert_eq!(at("t1", false).as_deref(), Some("t3"));
    assert_eq!(round(&keys(&["t1"]), &"t1".to_string(), true), None);
    assert_eq!(round(&sight, &"t9".to_string(), true), None);
}

#[test]
fn a_pane_closed_leaves_the_keys_to_the_one_used_last_else_its_neighbour() {
    let tree = tee().tree;
    let sight = keys(&["t1", "t2", "t3"]);
    // The one used last of the rest, whatever lies nearer.
    let taken = taker(&tree, &keys(&["t3", "t1", "t2"]), "t3", &sight);
    assert_eq!(taken.as_deref(), Some("t1"));
    // None used: the neighbour in its branch that takes its share, after it else before.
    assert_eq!(taker(&tree, &[], "t2", &sight).as_deref(), Some("t3"));
    assert_eq!(taker(&tree, &[], "t3", &sight).as_deref(), Some("t2"));
    // The left pane's neighbour is the column: its first pane.
    assert_eq!(taker(&tree, &[], "t1", &sight).as_deref(), Some("t2"));
    // One used that is no longer in sight is passed over.
    let gone = taker(&tree, &keys(&["t9"]), "t1", &sight);
    assert_eq!(gone.as_deref(), Some("t2"));
}
