//! Tests of `split`.

use super::*;

const A: Key = Key::Web(1);
const B: Key = Key::Term(2);
const C: Key = Key::Web(3);

const fn beside(key: Key, right: bool) -> Beside {
    Beside { key, right }
}

#[test]
fn the_shown_tab_is_one_half_and_the_one_beside_the_other() {
    assert_eq!(pair(Some(A), Some(beside(B, true)), |_| true), Some((A, B)));
    assert_eq!(
        pair(Some(A), Some(beside(B, false)), |_| true),
        Some((B, A))
    );
}

#[test]
fn no_split_without_a_tab_beside_still_there_and_other_than_the_shown() {
    assert_eq!(pair(Some(A), None, |_| true), None);
    assert_eq!(pair(None, Some(beside(B, true)), |_| true), None);
    assert_eq!(pair(Some(A), Some(beside(A, true)), |_| true), None);
    assert_eq!(pair(Some(A), Some(beside(B, true)), |key| key != B), None);
}

#[test]
fn picking_the_tab_beside_swaps_their_roles_in_place() {
    // B on the right, A shown on the left: B picked, A is beside, on the left.
    assert_eq!(
        after_show(Some(beside(B, true)), Some(A), B),
        Some(beside(A, false))
    );
    // The layout stays: A left, B right.
    assert_eq!(
        pair(Some(B), Some(beside(A, false)), |_| true),
        Some((A, B))
    );
}

#[test]
fn another_tab_picked_shows_whole() {
    // The project keeps the split (host_split.rs): the window shows the tab picked alone.
    assert_eq!(after_show(Some(beside(B, true)), Some(A), C), None);
    assert_eq!(after_show(None, Some(A), C), None);
}

#[test]
fn the_partner_is_the_next_tab_else_the_one_before() {
    assert_eq!(partner(&[A, B, C], A), Some(B));
    assert_eq!(partner(&[A, B, C], C), Some(B));
    assert_eq!(partner(&[A], A), None);
    assert_eq!(partner(&[B, C], A), None);
}

#[test]
fn the_halves_share_the_width_with_a_line_between() {
    let ((left, first), (right, second)) = halves(801.0, 52.0, 0.5);
    assert!((left - 52.0).abs() < f64::EPSILON);
    assert!((first - 400.0).abs() < f64::EPSILON);
    assert!((right - 453.0).abs() < f64::EPSILON);
    assert!((first + GAP + second - 801.0).abs() < f64::EPSILON);
    // Dragged: a quarter, and at the ends one half has it all.
    assert!((halves(801.0, 0.0, 0.25).0.1 - 200.0).abs() < f64::EPSILON);
    assert!((halves(801.0, 0.0, 1.0).1.1).abs() < f64::EPSILON);
}

#[test]
fn a_dragged_line_leaves_each_half_some_of_the_width() {
    assert!((held(0.05) - LEAST).abs() < f64::EPSILON);
    assert!((held(0.97) - (1.0 - LEAST)).abs() < f64::EPSILON);
    assert!((held(0.4) - 0.4).abs() < f64::EPSILON);
}

#[test]
fn a_kept_split_comes_back_around_either_of_its_tabs() {
    let kept = Kept {
        shown: "w1".to_string(),
        beside: "t2".to_string(),
        right: true,
    };
    // Back on the tab that was in use, the other is beside it as it was.
    assert_eq!(
        again(&kept, "w1"),
        Some(Beside {
            key: Key::Term(2),
            right: true
        })
    );
    // Back on the one that was beside, the one in use is on the other side of it.
    assert_eq!(
        again(&kept, "t2"),
        Some(Beside {
            key: Key::Web(1),
            right: false
        })
    );
    assert_eq!(again(&kept, "w3"), None);
}
