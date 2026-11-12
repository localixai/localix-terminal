//! Tests of a session's documents: a stack of panes with one in sight, in the tree and in the
//! store.

use crate::layout::*;
use crate::store::Store;

/// A terminal (1) with documents 2 and 3 on its right, 3 in sight.
fn session() -> Tree {
    let mut tree = Tree::leaf(1);
    assert!(tree.stack(2));
    assert!(tree.stack(3));
    tree
}

#[test]
fn the_first_document_of_a_pane_alone_is_a_stack_on_its_right() {
    let mut tree = Tree::leaf(1);
    assert!(tree.stack(2));
    assert_eq!(tree.panes(), [1, 2]);
    assert_eq!(tree.visible(), [1, 2]);
    assert_eq!(tree.stacked(), [2]);
}

#[test]
fn the_next_document_joins_the_stack_and_is_the_one_in_sight() {
    let tree = session();
    assert_eq!(tree.panes(), [1, 2, 3]);
    assert_eq!(tree.visible(), [1, 3]);
    assert_eq!(tree.stacked(), [2, 3]);
}

#[test]
fn a_document_shown_is_the_one_in_sight_and_the_rest_keep_their_order() {
    let mut tree = session();
    assert!(tree.show(2));
    assert_eq!(tree.visible(), [1, 2]);
    assert_eq!(tree.stacked(), [2, 3]);
    // A pane that is no document of it, or the terminal itself, changes nothing.
    assert!(!tree.show(9));
    assert!(!tree.show(1));
    assert_eq!(tree.visible(), [1, 2]);
}

#[test]
fn a_document_carried_along_its_row_goes_before_another_or_to_the_end() {
    let mut tree = session();
    assert!(tree.stack(4));
    assert!(tree.show(2));
    // Before another document, and to the end with none: the one in sight stays in sight.
    assert!(tree.place(4, Some(2)));
    assert_eq!(tree.stacked(), [4, 2, 3]);
    assert!(tree.place(4, None));
    assert_eq!(tree.stacked(), [2, 3, 4]);
    assert_eq!(tree.visible(), [1, 2]);
    // Where it is already, before itself, before what is no document of the stack, or a pane
    // that is no document: nothing moves.
    assert!(!tree.place(2, Some(3)));
    assert!(!tree.place(4, None));
    assert!(!tree.place(3, Some(3)));
    assert!(!tree.place(3, Some(1)));
    assert!(!tree.place(1, Some(2)));
    assert_eq!(tree.stacked(), [2, 3, 4]);
}

#[test]
fn the_document_in_sight_taken_out_shows_its_neighbour_and_the_last_ends_the_stack() {
    let mut tree = session();
    assert!(tree.stack(4));
    assert!(tree.show(3));
    let tree = tree.without(3).unwrap();
    assert_eq!(tree.visible(), [1, 4]);
    let tree = tree.without(4).unwrap();
    assert_eq!(tree.visible(), [1, 2]);
    let tree = tree.without(2).unwrap();
    assert_eq!(tree, Tree::leaf(1));
}

#[test]
fn a_session_closed_leaves_its_documents_a_stack_alone() {
    let tree = session().without(1).unwrap();
    assert_eq!(tree.stacked(), [2, 3]);
    assert_eq!(tree.visible(), [3]);
}

#[test]
fn a_document_replaced_keeps_its_place_and_whether_it_is_in_sight() {
    let mut tree = session();
    assert!(tree.replace(3, 7));
    assert_eq!(tree.stacked(), [2, 7]);
    assert_eq!(tree.visible(), [1, 7]);
}

#[test]
fn two_panes_side_by_side_have_no_room_for_a_stack() {
    let mut tree = Tree::leaf(1);
    assert!(tree.split(1, 5, true, true));
    assert!(!tree.stack(2));
    assert_eq!(tree.panes(), [1, 5]);
}

#[test]
fn a_pane_split_beside_a_document_is_beside_its_whole_stack() {
    let mut tree = Tree::stack_of(2);
    assert!(tree.stack(3));
    assert!(tree.split(2, 1, true, true));
    assert_eq!(tree.panes(), [2, 3, 1]);
    assert_eq!(tree.visible(), [3, 1]);
}

/// A store with project 1 and a workspace of it in a window.
fn store() -> (Store, WorkspaceId) {
    let store = Store::memory().unwrap();
    let space = store
        .write(|tx| {
            tx.execute(
                "INSERT INTO projects (id, path, name, created, opened) VALUES (1, '/p', 'p', 0, 0)",
                [],
            )
            .map_err(|why| why.to_string())?;
            let window = open_window(tx, 0)?;
            workspace_of(tx, window, 1, 0)
        })
        .unwrap();
    (store, space)
}

fn pane(store: &Store) -> crate::ledger::Row {
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
fn a_stack_is_kept_with_its_order_and_the_document_in_sight() {
    let (store, space) = store();
    let [term, one, two] = [pane(&store), pane(&store), pane(&store)];
    let mut tree = Tree::leaf(term);
    assert!(tree.stack(one));
    assert!(tree.stack(two));
    assert!(tree.show(one));
    let id = store
        .write(|tx| open_tab(tx, space, &tree, Some(term), None, 0))
        .unwrap();
    let kept = store.read(|db| tab(db, id)).unwrap().unwrap();
    assert_eq!(kept.tree, tree);
    assert_eq!(kept.focused, Some(term));
    assert!(!kept.whole);
    let all = store.read(|db| tabs(db, space)).unwrap();
    assert_eq!(all[0].tree, tree);
    // A document out of sight is of its tab all the same.
    assert_eq!(store.read(|db| tab_of(db, two)).unwrap(), Some(id));
}

#[test]
fn a_stack_of_one_document_is_still_a_stack() {
    let (store, space) = store();
    let one = pane(&store);
    let tree = Tree::stack_of(one);
    let id = store
        .write(|tx| open_tab(tx, space, &tree, None, None, 0))
        .unwrap();
    assert_eq!(store.read(|db| tab(db, id)).unwrap().unwrap().tree, tree);
}

#[test]
fn a_tab_made_whole_is_kept_so_and_its_pane_in_focus_must_be_in_sight() {
    let (store, space) = store();
    let [term, one, two] = [pane(&store), pane(&store), pane(&store)];
    let mut tree = Tree::leaf(term);
    assert!(tree.stack(one));
    assert!(tree.stack(two));
    let id = store
        .write(|tx| {
            // Document `one` is out of sight: the focus falls to a pane in sight.
            let id = open_tab(tx, space, &tree, Some(one), None, 0)?;
            set_whole(tx, id, true)?;
            Ok(id)
        })
        .unwrap();
    let kept = store.read(|db| tab(db, id)).unwrap().unwrap();
    assert!(kept.whole);
    assert_eq!(kept.focused, Some(term));
    assert!(store.read(|db| tabs(db, space)).unwrap()[0].whole);
}

#[test]
fn the_document_in_sight_is_named_and_the_halves_change_sides() {
    let mut tree = session();
    assert_eq!(tree.doc(), Some(3));
    assert_eq!(Tree::leaf(1).doc(), None);
    // The documents on the left: the two change sides, and keep their shares.
    assert!(tree.order(2));
    assert_eq!(tree.visible(), [3, 1]);
    assert!(!tree.order(2));
    assert!(tree.order(1));
    assert_eq!(tree.visible(), [1, 3]);
}
