//! Tests of `layout`: a tab's tree split, resized and taken apart; windows, workspaces and tabs
//! kept, moved and closed in the store.

use super::*;
use crate::store::Store;

fn shares(tree: &Tree) -> Vec<f64> {
    match tree {
        Tree::Leaf { .. } => vec![],
        Tree::Branch { children, .. } => children.iter().map(|child| child.share).collect(),
    }
}

#[test]
fn a_pane_split_beside_another_halves_it() {
    let mut tree = Tree::leaf(1);
    assert!(tree.split(1, 2, true, true));
    assert_eq!(tree.panes(), [1, 2]);
    assert_eq!(shares(&tree), [0.5, 0.5]);
    // The same way again: one more child, sharing the one it was put beside.
    assert!(tree.split(2, 3, true, false));
    assert_eq!(tree.panes(), [1, 3, 2]);
    assert_eq!(shares(&tree), [0.5, 0.25, 0.25]);
    // The other way: that pane becomes a branch of its own.
    assert!(tree.split(1, 4, false, true));
    assert_eq!(tree.panes(), [1, 4, 3, 2]);
    assert!(!tree.split(9, 5, true, true));
}

#[test]
fn a_pane_taken_out_gives_its_share_to_the_rest() {
    let mut tree = Tree::leaf(1);
    tree.split(1, 2, true, true);
    tree.split(2, 3, true, true);
    let tree = tree.without(2).unwrap();
    assert_eq!(tree.panes(), [1, 3]);
    let total: f64 = shares(&tree).iter().sum();
    assert!((total - 1.0).abs() < 1e-9);
    // A branch left with one pane is that pane.
    let alone = tree.without(3).unwrap();
    assert_eq!(alone, Tree::leaf(1));
    assert_eq!(alone.without(1), None);
}

#[test]
fn a_branch_resized_keeps_each_side_above_the_least() {
    let mut tree = Tree::leaf(1);
    tree.split(1, 2, true, true);
    assert!(tree.resize(&[], 1, 0.3));
    assert_eq!(shares(&tree), [0.3, 0.7]);
    assert!(tree.resize(&[], 1, 0.0));
    assert!((shares(&tree)[0] - LEAST).abs() < 1e-9);
    assert!(!tree.resize(&[0], 1, 0.5));
    assert!(!tree.resize(&[], 0, 0.5));
}

/// A store with project 1.
fn store() -> Store {
    let store = Store::memory().unwrap();
    store
        .write(|tx| {
            tx.execute(
                "INSERT INTO projects (id, path, name, created, opened) VALUES (1, '/p', 'p', 0, 0)",
                [],
            )
            .map(drop)
            .map_err(|why| why.to_string())
        })
        .unwrap();
    store
}

/// A pane of project 1: its row.
fn pane(store: &Store) -> PaneId {
    store
        .write(|tx| {
            let opened = crate::ledger::Opened {
                project: 1,
                kind: crate::ledger::Kind::Terminal,
                agent: None,
                title: "",
                url: None,
                cwd: None,
                command: None,
            };
            crate::ledger::open(tx, &opened, 0)
        })
        .unwrap()
}

#[test]
fn tabs_are_kept_in_their_places_with_their_trees() {
    let store = store();
    let (one, two, three) = (pane(&store), pane(&store), pane(&store));
    let (win, space, first, second) = store
        .write(|tx| {
            let window = open_window(tx, 0)?;
            let workspace = workspace_of(tx, window, 1, 0)?;
            assert_eq!(workspace_of(tx, window, 1, 0)?, workspace);
            let first = open_tab(tx, workspace, &Tree::leaf(one), None, None, 0)?;
            let mut split = Tree::leaf(two);
            split.split(two, three, true, true);
            let second = open_tab(tx, workspace, &split, Some(three), Some(0), 0)?;
            show_tab(tx, workspace, Some(second))?;
            show_workspace(tx, window, workspace)?;
            Ok((window, workspace, first, second))
        })
        .unwrap();
    let tabs = store.read(|db| tabs(db, space)).unwrap();
    assert_eq!(
        tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
        [second, first]
    );
    assert_eq!(tabs[0].tree.panes(), [two, three]);
    assert_eq!(tabs[0].focused, Some(three));
    assert_eq!(tabs[1].focused, Some(one));
    assert_eq!(store.read(|db| tab_of(db, three)).unwrap(), Some(second));
    let kept = store.read(|db| window(db, win)).unwrap().unwrap();
    assert_eq!(kept.shown, Some(space));
    let kept = store.read(|db| workspace(db, space)).unwrap().unwrap();
    assert_eq!(kept.shown, Some(second));
}

#[test]
fn a_tab_moves_to_another_window_whole_and_closes_whole() {
    let store = store();
    let (one, two) = (pane(&store), pane(&store));
    let (from, to, tab) = store
        .write(|tx| {
            let first = open_window(tx, 0)?;
            let second = open_window(tx, 0)?;
            let from = workspace_of(tx, first, 1, 0)?;
            let to = workspace_of(tx, second, 1, 0)?;
            let tab = open_tab(tx, from, &Tree::leaf(one), None, None, 0)?;
            open_tab(tx, from, &Tree::leaf(two), None, None, 0)?;
            show_tab(tx, from, Some(tab))?;
            move_tab(tx, tab, to, 0)?;
            Ok((from, to, tab))
        })
        .unwrap();
    let left = store.read(|db| tabs(db, from)).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].place, 0);
    assert_eq!(
        store.read(|db| workspace(db, from)).unwrap().unwrap().shown,
        None
    );
    let moved = store.read(|db| tabs(db, to)).unwrap();
    assert_eq!((moved[0].id, moved[0].tree.panes()), (tab, vec![one]));
    store.write(|tx| close_tab(tx, tab, 1)).unwrap();
    assert!(store.read(|db| tabs(db, to)).unwrap().is_empty());
    assert_eq!(store.read(|db| tab_of(db, one)).unwrap(), None);
}

#[test]
fn a_window_closed_closes_its_tabs() {
    let store = store();
    let one = pane(&store);
    let (win, space) = store
        .write(|tx| {
            let window = open_window(tx, 0)?;
            let workspace = workspace_of(tx, window, 1, 0)?;
            open_tab(tx, workspace, &Tree::leaf(one), None, None, 0)?;
            close_window(tx, window, 1)?;
            Ok((window, workspace))
        })
        .unwrap();
    assert!(store.read(|db| tabs(db, space)).unwrap().is_empty());
    assert!(
        store
            .read(windows)
            .unwrap()
            .iter()
            .all(|open| open.id != win)
    );
}
