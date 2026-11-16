//! Tests of where a tab's panes lie: rectangles, dividers, the pane beside, and what fits.

use crate::layout::geometry::{Rect, Side, Spacing, beside, fits, held, lay, neighbour};
use crate::layout::{Share, Tree};

const SPACING: Spacing = Spacing {
    gap: 1.0,
    least: 190.0,
};

fn branch(across: bool, children: Vec<(f64, Tree)>) -> Tree {
    let children = (children.into_iter())
        .map(|(share, tree)| Share { share, tree })
        .collect();
    Tree::Branch { across, children }
}

/// 1 on the left; on the right 2 over 3.
fn tee() -> Tree {
    let right = branch(false, vec![(0.5, Tree::leaf(2)), (0.5, Tree::leaf(3))]);
    branch(true, vec![(0.5, Tree::leaf(1)), (0.5, right)])
}

#[test]
fn two_panes_side_by_side_lie_as_the_halves_of_a_split_did() {
    // The halves of 1001 points from 248, a gap of one: 500 and 500, the second from 749.
    let tree = branch(true, vec![(0.5, Tree::leaf(1)), (0.5, Tree::leaf(2))]);
    let laid = lay(&tree, Rect::new(248.0, 38.0, 1001.0, 700.0), SPACING);
    assert_eq!(
        laid.panes,
        [
            (1, Rect::new(248.0, 38.0, 500.0, 700.0)),
            (2, Rect::new(749.0, 38.0, 500.0, 700.0)),
        ]
    );
    let [divider] = laid.dividers.as_slice() else {
        panic!("one divider: {:?}", laid.dividers)
    };
    assert_eq!(
        (divider.path.as_slice(), divider.at, divider.across),
        (&[][..], 1, true)
    );
    assert_eq!(divider.rect, Rect::new(748.0, 38.0, 1.0, 700.0));
    assert!((divider.room - 1000.0).abs() < 1e-9);
    // A third of it, rounded to a whole point.
    let tree = branch(true, vec![(0.35, Tree::leaf(1)), (0.65, Tree::leaf(2))]);
    let laid = lay(&tree, Rect::new(0.0, 0.0, 1001.0, 10.0), SPACING);
    assert_eq!(laid.rect(&1).map(|rect| rect.width), Some(350.0));
    assert_eq!(
        laid.rect(&2).map(|rect| (rect.x, rect.width)),
        Some((351.0, 650.0))
    );
}

#[test]
fn panes_cover_the_area_but_the_gaps_and_never_overlap() {
    let area = Rect::new(10.0, 20.0, 901.0, 601.0);
    let laid = lay(&tee(), area, SPACING);
    let rect = |pane| laid.rect(&pane).unwrap();
    assert_eq!(rect(1), Rect::new(10.0, 20.0, 450.0, 601.0));
    assert_eq!(rect(2), Rect::new(461.0, 20.0, 450.0, 300.0));
    assert_eq!(rect(3), Rect::new(461.0, 321.0, 450.0, 300.0));
    let covered: f64 = laid.panes.iter().map(|(_, r)| r.width * r.height).sum();
    let gaps = 601.0 + 450.0;
    assert!((covered + gaps - area.width * area.height).abs() < 1e-6);
    // The root's divider runs the whole height; the column's crosses the column alone.
    let lines: Vec<_> = (laid.dividers.iter())
        .map(|divider| (divider.path.clone(), divider.across, divider.rect))
        .collect();
    assert_eq!(
        lines,
        [
            (vec![], true, Rect::new(460.0, 20.0, 1.0, 601.0)),
            (vec![1], false, Rect::new(461.0, 320.0, 450.0, 1.0)),
        ]
    );
}

#[test]
fn a_pane_keeps_its_least_points_while_there_is_room() {
    // A share kept from a wide window, or a drag too far, never leaves a pane under 190.
    let tree = branch(true, vec![(0.05, Tree::leaf(1)), (0.95, Tree::leaf(2))]);
    let laid = lay(&tree, Rect::new(0.0, 0.0, 1001.0, 10.0), SPACING);
    assert_eq!(laid.rect(&1).map(|rect| rect.width), Some(190.0));
    // A pair beside a pane needs two panes' least.
    let shares = held(&[0.9, 0.1], &[190.0, 381.0], 1000.0);
    assert!((shares[1] - 0.381).abs() < 1e-9, "{shares:?}");
    // Too narrow for both: each a part by what it needs.
    let shares = held(&[0.9, 0.1], &[190.0, 190.0], 300.0);
    assert_eq!(shares, [0.5, 0.5]);
    // A tenth at the least however wide.
    let shares = held(&[0.99, 0.01], &[190.0, 190.0], 10_000.0);
    assert!((shares[1] - 0.1).abs() < 1e-9);
    // Three, one short: it is given its least, the others keep their proportions.
    let shares = held(&[0.6, 0.3, 0.1], &[190.0; 3], 1000.0);
    assert!((shares[2] - 0.19).abs() < 1e-9);
    assert!((shares[0] / shares[1] - 2.0).abs() < 1e-9);
}

#[test]
fn the_pane_beside_is_found_by_where_it_lies() {
    let laid = lay(&tee(), Rect::new(0.0, 0.0, 901.0, 601.0), SPACING);
    let beside = |from, side, recent: &[i64]| neighbour(&laid.panes, &from, side, recent, 1.0);
    assert_eq!(beside(2, Side::Down, &[]), Some(3));
    assert_eq!(beside(3, Side::Up, &[]), Some(2));
    assert_eq!(beside(2, Side::Left, &[]), Some(1));
    assert_eq!(beside(3, Side::Left, &[]), Some(1));
    // From 1, both 2 and 3 lie on its right: the one used last wins, else the upper (as long
    // alongside, nearer the middle by a point).
    assert_eq!(beside(1, Side::Right, &[3, 2]), Some(3));
    assert_eq!(beside(1, Side::Right, &[2, 3]), Some(2));
    // Nothing wraps round at the tab's edge.
    assert_eq!(beside(1, Side::Left, &[]), None);
    assert_eq!(beside(2, Side::Up, &[]), None);
    assert_eq!(beside(3, Side::Right, &[]), None);
    assert_eq!(beside(9, Side::Right, &[]), None);
}

#[test]
fn every_pane_on_a_side_is_beside_it_in_the_order_they_lie() {
    let laid = lay(&tee(), Rect::new(0.0, 0.0, 901.0, 601.0), SPACING);
    let on = |from, side| beside(&laid.panes, &from, side, 1.0);
    assert_eq!(on(1, Side::Right), [2, 3]);
    assert_eq!(on(2, Side::Left), [1]);
    assert_eq!(on(3, Side::Up), [2]);
    assert!(on(1, Side::Left).is_empty());
    assert!(on(9, Side::Right).is_empty());
}

#[test]
fn the_pane_alongside_most_wins_a_tie_of_none_used() {
    // 1 over 2 on the left, one tall 3 on the right; 1 takes two thirds of the column.
    let left = branch(false, vec![(0.66, Tree::leaf(1)), (0.34, Tree::leaf(2))]);
    let tree = branch(true, vec![(0.5, left), (0.5, Tree::leaf(3))]);
    let laid = lay(&tree, Rect::new(0.0, 0.0, 901.0, 901.0), SPACING);
    assert_eq!(neighbour(&laid.panes, &3, Side::Left, &[], 1.0), Some(1));
    assert_eq!(neighbour(&laid.panes, &3, Side::Left, &[2], 1.0), Some(2));
}

#[test]
fn a_pane_splits_only_where_both_parts_keep_their_least() {
    let laid = lay(&tee(), Rect::new(0.0, 0.0, 901.0, 601.0), SPACING);
    assert!(
        fits(&laid, &1, true, SPACING),
        "450 points across hold two of 190"
    );
    assert!(!fits(&laid, &2, false, SPACING), "300 points high do not");
    assert!(fits(&laid, &1, false, SPACING));
    assert!(!fits(&laid, &9, true, SPACING), "no such pane");
    let narrow = lay(&tee(), Rect::new(0.0, 0.0, 700.0, 900.0), SPACING);
    assert!(!fits(&narrow, &1, true, SPACING));
}
