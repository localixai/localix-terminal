//! Tests of `browser::glide`: where a pane that opens starts, and how far from where it goes a
//! view's layer is drawn as its spring starts.

use super::{Frame, Offset, offset, shut, toward};
use crate::browser::Side;

/// The frame a layer is drawn over when its view stands at `after` and `part` of `offset` is
/// still to go, for a layer placed by its origin whose parent counts from the top.
fn drawn_at(after: Frame, offset: Offset, part: f64) -> Frame {
    [
        after[0] + offset.place.0 * part,
        after[1] + offset.place.1 * part,
        after[2] + offset.size.0 * part,
        after[3] + offset.size.1 * part,
    ]
}

#[test]
fn a_pane_opens_from_its_own_edge_on_the_side_it_lands() {
    let frame = [100.0, 40.0, 300.0, 200.0];
    assert_eq!(shut(frame, Side::Left, true), [100.0, 40.0, 0.0, 200.0]);
    assert_eq!(shut(frame, Side::Right, true), [400.0, 40.0, 0.0, 200.0]);
    assert_eq!(shut(frame, Side::Up, true), [100.0, 40.0, 300.0, 0.0]);
    assert_eq!(shut(frame, Side::Down, true), [100.0, 240.0, 300.0, 0.0]);
}

#[test]
fn a_parent_that_counts_from_its_bottom_has_its_top_edge_at_the_far_end() {
    let frame = [100.0, 40.0, 300.0, 200.0];
    assert_eq!(shut(frame, Side::Up, false), [100.0, 240.0, 300.0, 0.0]);
    assert_eq!(shut(frame, Side::Down, false), [100.0, 40.0, 300.0, 0.0]);
    assert_eq!(shut(frame, Side::Left, false), [100.0, 40.0, 0.0, 200.0]);
}

#[test]
fn a_divider_comes_from_the_side_its_pane_opens_at() {
    let line = [296.0, 0.0, 9.0, 400.0];
    assert_eq!(
        toward(line, Side::Right, 301.0, true),
        [597.0, 0.0, 9.0, 400.0]
    );
    assert_eq!(
        toward(line, Side::Left, 301.0, true),
        [-5.0, 0.0, 9.0, 400.0]
    );
    let line = [0.0, 196.0, 600.0, 9.0];
    assert_eq!(
        toward(line, Side::Down, 201.0, true),
        [0.0, 397.0, 600.0, 9.0]
    );
    assert_eq!(toward(line, Side::Up, 201.0, true), [0.0, -5.0, 600.0, 9.0]);
    // Down a parent that counts from its bottom is toward nothing.
    assert_eq!(
        toward(line, Side::Down, 201.0, false),
        [0.0, -5.0, 600.0, 9.0]
    );
    assert_eq!(
        toward(line, Side::Up, 201.0, false),
        [0.0, 397.0, 600.0, 9.0]
    );
}

#[test]
fn a_view_that_stands_where_it_stood_is_drawn_there() {
    let frame = [10.0, 20.0, 300.0, 200.0];
    assert!(offset(frame, frame, (0.0, 0.0), true).none());
    assert!(offset(frame, frame, (0.5, 0.5), false).none());
    assert!(!offset(frame, [10.0, 20.0, 150.0, 200.0], (0.0, 0.0), true).none());
}

#[test]
fn a_layer_starts_over_where_its_view_stood() {
    let (before, after) = ([0.0, 0.0, 600.0, 400.0], [300.0, 0.0, 300.0, 400.0]);
    let from_origin = offset(before, after, (0.0, 0.0), true);
    assert_eq!(from_origin.place, (-300.0, 0.0));
    assert_eq!(from_origin.begin, (0.0, 0.0));
    assert_eq!(from_origin.size, (300.0, 0.0));
    assert_eq!(drawn_at(after, from_origin, 1.0), before);
    assert_eq!(drawn_at(after, from_origin, 0.0), after);
    // Placed by its middle, a layer's place moves half as far as it grows, as well.
    let from_middle = offset(before, after, (0.5, 0.5), true);
    assert_eq!(from_middle.place, (-150.0, 0.0));
    assert_eq!(from_middle.size, from_origin.size);
}

#[test]
fn what_a_layer_counted_from_its_bottom_holds_keeps_its_place_from_its_top() {
    // Grown shorter by 150: its bounds begin 150 lower as the spring starts, so the top of what
    // it holds is drawn at the top of where it stood.
    let (before, after) = ([0.0, 0.0, 600.0, 400.0], [0.0, 150.0, 600.0, 250.0]);
    let moved = offset(before, after, (0.0, 0.0), false);
    assert_eq!((moved.begin, moved.size), ((0.0, -150.0), (0.0, 150.0)));
    assert_eq!(moved.place, (0.0, -150.0));
    let top = offset(before, after, (0.0, 0.0), true);
    assert_eq!((top.begin, top.size), ((0.0, 0.0), (0.0, 150.0)));
}

#[test]
fn the_pane_that_makes_room_and_the_one_that_opens_keep_their_edges_together() {
    // A pane over the whole part, a tab landing on each side of it with a point between them.
    let whole = [0.0, 0.0, 601.0, 401.0];
    let cases = [
        (
            Side::Right,
            [0.0, 0.0, 300.0, 401.0],
            [301.0, 0.0, 300.0, 401.0],
        ),
        (
            Side::Left,
            [301.0, 0.0, 300.0, 401.0],
            [0.0, 0.0, 300.0, 401.0],
        ),
        (
            Side::Down,
            [0.0, 0.0, 601.0, 200.0],
            [0.0, 201.0, 601.0, 200.0],
        ),
        (
            Side::Up,
            [0.0, 201.0, 601.0, 200.0],
            [0.0, 0.0, 601.0, 200.0],
        ),
    ];
    for (side, beside, landed) in cases {
        let room = offset(whole, beside, (0.0, 0.0), true);
        let opens = offset(shut(landed, side, true), landed, (0.0, 0.0), true);
        for part in [1.0, 0.7, 0.3, 0.0] {
            let (stays, comes) = (drawn_at(beside, room, part), drawn_at(landed, opens, part));
            let between = match side {
                Side::Right => comes[0] - (stays[0] + stays[2]),
                Side::Left => stays[0] - (comes[0] + comes[2]),
                Side::Down => comes[1] - (stays[1] + stays[3]),
                Side::Up => stays[1] - (comes[1] + comes[3]),
            };
            // The point between them opens with the spring: never one over the other, never
            // further apart than they end.
            assert!(
                (between - (1.0 - part)).abs() < 1e-9,
                "{side:?} at {part}: {between} between {stays:?} and {comes:?}"
            );
        }
    }
}

#[test]
fn a_pane_that_opens_is_never_drawn_past_where_it_stands() {
    // Its bounds grow from nothing: their size starts as much under nothing as it is wide,
    // and they begin where they always do, so what it holds is drawn in its place all the way.
    let landed = [301.0, 0.0, 300.0, 401.0];
    let opens = offset(shut(landed, Side::Right, true), landed, (0.0, 0.0), true);
    assert_eq!(opens.size, (-300.0, 0.0));
    assert_eq!(opens.begin, (0.0, 0.0));
    assert_eq!(opens.place, (300.0, 0.0));
    for part in [1.0, 0.5, 0.0] {
        let now = drawn_at(landed, opens, part);
        assert!(now[0] >= landed[0] && now[0] + now[2] <= landed[0] + landed[2] + 1e-9);
    }
}
