//! Tests of `split`.

use super::*;

fn w(id: u32) -> Key {
    Key::Web(id)
}
fn t(id: u32) -> Key {
    Key::Term(id)
}

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
    // Each half keeps its least, in points, on any room.
    assert!((held(0.05, 1000.0) - 0.19).abs() < 1e-9);
    assert!((held(0.97, 1000.0) - 0.81).abs() < 1e-9);
    assert!((held(0.4, 1000.0) - 0.4).abs() < f64::EPSILON);
    assert!((held(0.05, 2000.0) - crate::layout::LEAST).abs() < 1e-9);
    // Too narrow for both: half each.
    assert!((held(0.1, 300.0) - 0.5).abs() < f64::EPSILON);
}

#[test]
fn a_saved_ratio_is_restored_without_an_old_projection_undoing_a_drag() {
    let mut split = Split::default();
    split.follow_share(Some((w(1), t(2))), Some(0.65));
    assert_eq!(split.share, 0.65);
    split.share = 0.7;
    split.sizing = Some(([w(1), t(2)], 0.7));
    split.follow_share(Some((w(1), t(2))), Some(0.65));
    assert_eq!(split.share, 0.7);
    assert!(split.sizing.is_some());
    split.follow_share(Some((w(1), t(2))), Some(0.7));
    assert!(split.sizing.is_none());
    split.follow_share(Some((w(1), t(2))), Some(0.6));
    assert_eq!(split.share, 0.6);
}

#[test]
fn changing_the_pair_forgets_a_pending_size_and_bad_saved_shares_are_ignored() {
    let mut split = Split {
        sizing: Some(([w(1), t(2)], 0.7)),
        ..Split::default()
    };
    split.follow_share(Some((w(3), t(4))), Some(0.4));
    assert_eq!(split.share, 0.4);
    assert!(split.sizing.is_none());
    for shares in [
        vec![f64::NAN, 0.5],
        vec![f64::INFINITY, 0.5],
        vec![f64::MAX, f64::MAX],
        vec![0.0, 1.0],
    ] {
        let arranged = crate::browser::Arranged {
            window: 1,
            group: "a".into(),
            shown: Some("w1".into()),
            tabs: vec![crate::browser::Arranging {
                panes: vec!["w1".into(), "t2".into()],
                across: true,
                shares,
                focused: Some("w1".into()),
                ..Default::default()
            }],
        };
        assert_eq!(share_of(&arranged, (w(1), t(2))), None);
    }
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

#[test]
fn a_carried_tab_lands_beside_a_whole_window_or_in_the_half_not_in_use() {
    use super::{Beside, landed};
    let (shown, other, third) = (Key::Term(1), Key::Web(2), Key::Term(3));
    // Whole: beside the shown tab, on the side it is carried to; the shown tab itself lands nowhere.
    assert_eq!(
        landed(None, Some(shown), None, other, true),
        Some(Beside {
            key: other,
            right: true
        })
    );
    assert_eq!(landed(None, Some(shown), None, shown, false), None);
    // Split, the shown tab on the left: another tab takes the right half, the one not in use.
    let beside = Some(Beside {
        key: other,
        right: true,
    });
    let pair = Some((shown, other));
    assert_eq!(
        landed(beside, Some(shown), pair, third, true),
        Some(Beside {
            key: third,
            right: true
        })
    );
    // Over the half in use: nothing laid out before it is let go of.
    assert_eq!(landed(beside, Some(shown), pair, third, false), None);
    // The shown tab carried over the other half: the two change sides.
    assert_eq!(
        landed(beside, Some(shown), pair, shown, true),
        Some(Beside {
            key: other,
            right: false
        })
    );
}

fn kept(shown: &str, beside: &str, right: bool) -> Kept {
    Kept {
        shown: shown.into(),
        beside: beside.into(),
        right,
    }
}

#[test]
fn a_window_drawn_as_the_layout_kept_it_follows_what_the_layout_keeps_now() {
    let drawn = kept("t1", "w1", true);
    // The app put another pane beside the terminal: the window shows that one there.
    let now = kept("t1", "w2", true);
    assert_eq!(
        followed(Some(&drawn), Some(&drawn), Some(&now), "t1"),
        Follow::Beside(beside(w(2), true))
    );
    // Seen from the other half it is the same window: the pane in use stays in use.
    let other = kept("w1", "t1", false);
    assert_eq!(
        followed(Some(&other), Some(&drawn), Some(&now), "w1"),
        Follow::Whole
    );
    // The layout keeps the tab whole now, or split elsewhere: the window is whole.
    assert_eq!(
        followed(Some(&drawn), Some(&drawn), None, "t1"),
        Follow::Whole
    );
    let elsewhere = kept("t5", "w1", true);
    assert_eq!(
        followed(Some(&drawn), Some(&drawn), Some(&elsewhere), "t1"),
        Follow::Whole
    );
}

#[test]
fn a_split_the_person_just_made_is_not_undone_by_what_the_layout_kept_before() {
    let drawn = kept("t1", "w1", true);
    let theirs = kept("t1", "t3", true);
    // The layout has not heard of it yet: it still keeps the one before.
    assert_eq!(
        followed(Some(&theirs), Some(&drawn), Some(&drawn), "t1"),
        Follow::Stay
    );
    // And one that says something else again is no reason to drop the person's.
    let late = kept("t1", "w2", true);
    assert_eq!(
        followed(Some(&theirs), Some(&drawn), Some(&late), "t1"),
        Follow::Stay
    );
    assert_eq!(
        followed(Some(&theirs), None, Some(&late), "t1"),
        Follow::Stay
    );
    // Nothing changed in the layout: nothing to follow. A whole window is the layout's to
    // split as it keeps (`again`), not this.
    assert_eq!(
        followed(Some(&drawn), Some(&drawn), Some(&drawn), "t1"),
        Follow::Stay
    );
    assert_eq!(followed(None, None, Some(&late), "t1"), Follow::Stay);
}

#[test]
fn a_split_stays_under_a_list_open_over_it_and_stands_aside_where_nothing_lies_over_pages() {
    let halves = Some(["t1", "f2"]);
    // Nothing is open: the window is as it is.
    assert_eq!(under(halves, false, true), halves);
    assert_eq!(under(halves, false, false), halves);
    // A menu, a dialog or an address's suggestions over the pages: both halves stay under it.
    assert_eq!(under(halves, true, true), halves);
    // Where the strip cannot lie over the pages, they stand aside for the list.
    assert_eq!(under(halves, true, false), None);
    // A whole window has no halves either way.
    assert_eq!(under(None::<[&str; 2]>, true, true), None);
}

#[test]
fn the_toolbar_spans_the_half_in_use_and_the_whole_part_of_a_window_not_split() {
    let halves = Some([("t1", 280.0, 509.0), ("w1", 790.0, 510.0)]);
    // Over the page in the right half: its left and width, where its menus and lists open.
    assert_eq!(span(halves, Some("w1"), (280.0, 1020.0)), (790.0, 510.0));
    assert_eq!(span(halves, Some("t1"), (280.0, 1020.0)), (280.0, 509.0));
    // Whole, or showing a tab in neither half: the pages' whole part.
    assert_eq!(span(None, Some("w1"), (280.0, 1020.0)), (280.0, 1020.0));
    assert_eq!(span(halves, Some("w9"), (280.0, 1020.0)), (280.0, 1020.0));
    assert_eq!(span(halves, None, (280.0, 1020.0)), (280.0, 1020.0));
}
