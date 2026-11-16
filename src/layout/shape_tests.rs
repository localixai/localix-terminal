//! Tests of what a tree holds along each way: shares made even, the path to a pane, a divider
//! moved a step, and the tree named by other ids.

use crate::layout::geometry::Side;
use crate::layout::{Share, Tree};

fn branch(across: bool, children: Vec<(f64, Tree)>) -> Tree {
    let children = (children.into_iter())
        .map(|(share, tree)| Share { share, tree })
        .collect();
    Tree::Branch { across, children }
}

/// 1 on the left; on the right 2 over 3: a T on its side.
fn tee() -> Tree {
    let right = branch(false, vec![(0.5, Tree::leaf(2)), (0.5, Tree::leaf(3))]);
    branch(true, vec![(0.5, Tree::leaf(1)), (0.5, right)])
}

fn shares<P>(tree: &Tree<P>) -> Vec<f64> {
    match tree {
        Tree::Branch { children, .. } => children.iter().map(|child| child.share).collect(),
        Tree::Leaf { .. } | Tree::Stack { .. } => vec![],
    }
}

fn near(one: &[f64], other: &[f64]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(a, b)| (a - b).abs() < 1e-9)
}

#[test]
fn a_tree_spans_its_panes_along_its_own_way_and_its_widest_child_across_it() {
    assert_eq!(Tree::leaf(1).span(true), 1);
    assert_eq!(tee().span(true), 2);
    assert_eq!(tee().span(false), 2);
    let three = branch(true, vec![(0.3, Tree::leaf(1)), (0.7, tee())]);
    assert_eq!(three.span(true), 3);
}

#[test]
fn even_shares_follow_how_many_panes_each_child_lays_along_its_branch() {
    // A pane beside a pair of the same way takes a third.
    let pair = branch(true, vec![(0.9, Tree::leaf(2)), (0.1, Tree::leaf(3))]);
    let mut tree = branch(true, vec![(0.8, Tree::leaf(1)), (0.2, pair)]);
    assert!(tree.equalize());
    assert!(near(&shares(&tree), &[1.0 / 3.0, 2.0 / 3.0]));
    let Tree::Branch { children, .. } = &tree else {
        panic!("not a branch")
    };
    assert!(near(&shares(&children[1].tree), &[0.5, 0.5]));
    // A pane beside two one above the other takes half: they lay one pane across.
    let mut tee = tee();
    tee.resize(&[], 1, 0.8);
    assert!(tee.equalize());
    assert!(near(&shares(&tee), &[0.5, 0.5]));
    assert!(!tee.equalize(), "even again changes nothing");
}

#[test]
fn a_panes_path_is_the_place_of_each_child_down_to_it() {
    let tree = tee();
    assert_eq!(tree.path_of(&1), Some(vec![0]));
    assert_eq!(tree.path_of(&3), Some(vec![1, 1]));
    assert_eq!(tree.path_of(&9), None);
    assert_eq!(Tree::leaf(4).path_of(&4), Some(vec![]));
    let mut session = Tree::leaf(1);
    session.stack(2);
    session.stack(3);
    assert_eq!(
        session.path_of(&2),
        Some(vec![1]),
        "a document is its stack's"
    );
    assert_eq!(
        tree.branch(&[1]).map(|(across, kids)| (across, kids.len())),
        Some((false, 2))
    );
    assert!(tree.branch(&[0]).is_none(), "a pane is no branch");
}

#[test]
fn a_step_moves_the_nearest_divider_of_its_way_beside_the_pane() {
    let mut tree = tee();
    // 3 grows up into 2: the divider of the right column moves up.
    assert!(tree.step(&3, Side::Up, 0.1));
    let Tree::Branch { children, .. } = &tree else {
        panic!("not a branch")
    };
    assert!(near(&shares(&children[1].tree), &[0.4, 0.6]));
    // 3 to the left: the column's own branch is of the other way, the root's divider moves.
    assert!(tree.step(&3, Side::Left, 0.1));
    assert!(near(&shares(&tree), &[0.4, 0.6]));
    // 1 to the right moves the same divider back.
    assert!(tree.step(&1, Side::Right, 0.1));
    assert!(near(&shares(&tree), &[0.5, 0.5]));
}

#[test]
fn a_step_names_its_divider_and_the_share_before_it_and_the_way_back_is_opposite() {
    let tree = tee();
    // 2 down: the right column's divider, its first child's share grown.
    let (path, at, share) = tree.stepped(&2, Side::Down, 0.1).unwrap();
    assert_eq!((path.as_slice(), at), (&[1][..], 1));
    assert!((share - 0.6).abs() < 1e-9);
    // 1 to the right: the root's divider.
    let (path, at, share) = tree.stepped(&1, Side::Right, 0.05).unwrap();
    assert_eq!((path.as_slice(), at), (&[][..], 1));
    assert!((share - 0.55).abs() < 1e-9);
    assert_eq!(tree.stepped(&1, Side::Left, 0.1), None);
    for side in [Side::Left, Side::Right, Side::Up, Side::Down] {
        assert_eq!(side.opposite().opposite(), side);
        assert_eq!(side.opposite().across(), side.across());
        assert_ne!(side.opposite().forward(), side.forward());
    }
}

#[test]
fn a_pane_at_the_tabs_edge_has_no_divider_to_move_that_way() {
    let mut tree = tee();
    let was = tree.clone();
    assert!(!tree.step(&1, Side::Left, 0.1));
    assert!(!tree.step(&2, Side::Right, 0.1));
    assert!(!tree.step(&2, Side::Up, 0.1));
    assert!(!tree.step(&1, Side::Down, 0.1), "1 is alone in its column");
    assert!(!tree.step(&9, Side::Right, 0.1));
    assert_eq!(tree, was);
}

#[test]
fn a_step_stops_at_the_least_share() {
    let mut tree = tee();
    for _ in 0..20 {
        tree.step(&1, Side::Right, 0.2);
    }
    assert!(near(&shares(&tree), &[0.9, 0.1]));
    assert!(
        !tree.step(&1, Side::Right, 0.2),
        "held at the least, it moves no more"
    );
}

#[test]
fn a_tree_named_by_other_ids_leaves_out_the_panes_with_none() {
    let name = |pane: &i64| (*pane != 2).then(|| format!("t{pane}"));
    let named = tee().named(&name).unwrap();
    assert_eq!(named.panes(), ["t1", "t3"]);
    assert!(
        near(&shares(&named), &[0.5, 0.5]),
        "the column of one is its pane"
    );
    assert_eq!(Tree::leaf(2).named(&name), None);
    let mut session = Tree::leaf(1);
    session.stack(4);
    session.stack(2);
    let named = session.named(&name).unwrap();
    assert_eq!(
        named.visible(),
        ["t1", "t4"],
        "a stack shows its next named pane"
    );
}
