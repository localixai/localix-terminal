//! Tests of `carry_zones`: where a carried tab lands as the pointer walks over the panes.

use std::sync::atomic::{AtomicU32, Ordering};

use super::{Side, Zones};
use crate::layout::geometry::Rect;

/// One pane, 600 by 400, from 100 across and 50 down.
fn one() -> Zones<u8, u32> {
    let pane = Rect::new(100.0, 50.0, 600.0, 400.0);
    Zones::new(vec![(1, pane)], pane)
}

/// Two panes side by side, a point apart.
fn two() -> Zones<u8, u32> {
    let area = Rect::new(0.0, 0.0, 601.0, 400.0);
    let left = Rect::new(0.0, 0.0, 300.0, 400.0);
    let right = Rect::new(301.0, 0.0, 300.0, 400.0);
    Zones::new(vec![(1, left), (2, right)], area)
}

/// The targets a walk over `points` takes, each once in the order taken.
fn walk(zones: &mut Zones<u8, u32>, points: impl Iterator<Item = (f64, f64)>) -> Vec<(u8, Side)> {
    let mut seen = vec![];
    for (x, y) in points {
        zones.at(x, y);
        if let Some(target) = zones.target()
            && seen.last() != Some(&target)
        {
            seen.push(target);
        }
    }
    seen
}

/// The side a tab that comes over pane `one()` at `across`, `down` of it (parts of its width
/// and height) lands on.
fn fresh(across: f64, down: f64) -> Option<Side> {
    let mut zones = one();
    zones.at(100.0 + across * 600.0, 50.0 + down * 400.0);
    zones.target().map(|(_, side)| side)
}

#[test]
fn a_pane_s_halves_are_its_left_and_its_right_down_to_near_its_upper_and_lower_edge() {
    // To the right of the middle and well under it: the right, not the pane's lower edge.
    assert_eq!(fresh(0.7, 0.8), Some(Side::Right));
    assert_eq!(fresh(0.6, 0.7), Some(Side::Right));
    assert_eq!(fresh(0.9, 0.2), Some(Side::Right));
    assert_eq!(fresh(0.45, 0.7), Some(Side::Left));
    assert_eq!(fresh(0.3, 0.25), Some(Side::Left));
    // The middle is no dead place: a half is taken there too.
    assert_eq!(fresh(0.55, 0.5), Some(Side::Right));
    assert_eq!(fresh(0.45, 0.5), Some(Side::Left));
    // A corner is its side's, not the edge's above or under it.
    assert_eq!(fresh(0.05, 0.95), Some(Side::Left));
    assert_eq!(fresh(0.96, 0.04), Some(Side::Right));
}

#[test]
fn over_and_under_are_taken_near_the_pane_s_upper_and_lower_edge() {
    // Near the edge, almost its whole width; a quarter of the way in, only at the middle.
    assert_eq!(fresh(0.5, 0.9), Some(Side::Down));
    assert_eq!(fresh(0.2, 0.95), Some(Side::Down));
    assert_eq!(fresh(0.8, 0.94), Some(Side::Down));
    assert_eq!(fresh(0.5, 0.1), Some(Side::Up));
    assert_eq!(fresh(0.15, 0.04), Some(Side::Up));
    assert_eq!(fresh(0.5, 0.78), Some(Side::Down));
    // Further in than a quarter of the pane's height, never.
    for across in [0.1, 0.3, 0.5, 0.7, 0.9] {
        for down in [0.26, 0.4, 0.5, 0.6, 0.74] {
            let side = fresh(across, down);
            assert!(
                matches!(side, Some(Side::Left | Side::Right)),
                "{across} across, {down} down: {side:?}"
            );
        }
    }
}

#[test]
fn a_walk_across_a_pane_a_point_a_step_goes_from_its_left_to_its_right_and_never_back() {
    let mut zones = one();
    let across = (100..700).map(|x| (f64::from(x), 250.0));
    assert_eq!(
        walk(&mut zones, across),
        vec![(1, Side::Left), (1, Side::Right)]
    );
    // Down the pane a third of the way across: its upper edge, its left, its lower edge.
    let mut zones = one();
    let down = (50..450).map(|y| (300.0, f64::from(y)));
    assert_eq!(
        walk(&mut zones, down),
        vec![(1, Side::Up), (1, Side::Left), (1, Side::Down)]
    );
}

#[test]
fn a_hand_that_shakes_where_two_zones_meet_changes_nothing() {
    let shake = [
        (-3.0, 3.0),
        (3.0, -3.0),
        (-3.0, -3.0),
        (3.0, 3.0),
        (0.0, 3.0),
        (3.0, 0.0),
    ];
    // Where the left's zone meets the upper edge's: a fifth across, a tenth down.
    let mut zones = one();
    zones.at(220.0, 90.0);
    let first = zones.target();
    assert!(first.is_some());
    for (dx, dy) in shake.iter().cycle().take(60) {
        zones.at(220.0 + dx, 90.0 + dy);
        assert_eq!(zones.target(), first);
    }
    // On the line between the halves: the half taken first stays, whichever it is.
    for (from, side) in [(110.0, Side::Left), (690.0, Side::Right)] {
        let mut zones = one();
        zones.at(from, 250.0);
        for (dx, dy) in shake.iter().cycle().take(60) {
            zones.at(400.0 + dx, 250.0 + dy);
            assert_eq!(zones.target(), Some((1, side)));
        }
    }
}

#[test]
fn another_side_is_taken_only_once_the_pointer_is_in_its_zone_by_the_margin() {
    // 400 points is the shorter side: the margin is a twentieth of it, 20 points.
    let mut zones = one();
    zones.at(160.0, 170.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    // A fifth across, the upper edge's zone begins 40 points down: 10 points into it is not
    // enough, 30 is.
    zones.at(220.0, 80.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    zones.at(220.0, 60.0);
    assert_eq!(zones.target(), Some((1, Side::Up)));
    // And out of it the same: 10 points under it the upper edge stays, 30 under the left is back.
    zones.at(220.0, 100.0);
    assert_eq!(zones.target(), Some((1, Side::Up)));
    zones.at(220.0, 120.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    // Over the line between the halves: 10 points past it the left stays, 25 past is the right.
    zones.at(390.0, 250.0);
    zones.at(410.0, 250.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    zones.at(425.0, 250.0);
    assert_eq!(zones.target(), Some((1, Side::Right)));
    // Past the margin into an edge's zone from the other half, the edge is taken at once, and
    // out of it the half the pointer is in.
    zones.at(300.0, 440.0);
    assert_eq!(zones.target(), Some((1, Side::Down)));
    zones.at(300.0, 300.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    // A small pane's margin is 8 points at least.
    let small = Rect::new(0.0, 0.0, 100.0, 100.0);
    let mut zones: Zones<u8, u32> = Zones::new(vec![(1, small)], small);
    zones.at(5.0, 50.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    zones.at(30.0, 10.0);
    assert_eq!(zones.target(), Some((1, Side::Left)));
    zones.at(30.0, 5.0);
    assert_eq!(zones.target(), Some((1, Side::Up)));
}

#[test]
fn another_pane_is_taken_past_the_divider_by_eight_points_and_a_walk_back_comes_back_the_same() {
    let mut zones = two();
    let across: Vec<(f64, f64)> = (0..601).map(|x| (f64::from(x), 200.0)).collect();
    let there = walk(&mut zones, across.iter().copied());
    let sides = vec![
        (1, Side::Left),
        (1, Side::Right),
        (2, Side::Left),
        (2, Side::Right),
    ];
    assert_eq!(there, sides);
    // Within eight points of the divider, the pane it came from is kept.
    let mut zones = two();
    zones.at(290.0, 200.0);
    zones.at(305.0, 200.0);
    assert_eq!(zones.target(), Some((1, Side::Right)));
    zones.at(310.0, 200.0);
    assert_eq!(zones.target(), Some((2, Side::Left)));
    // The way back is the same zones in the other order: none is left and taken again.
    let mut zones = two();
    let back = walk(&mut zones, across.into_iter().rev());
    let mut sides = sides;
    sides.reverse();
    assert_eq!(back, sides);
}

#[test]
fn off_the_panes_nothing_is_taken_and_coming_back_takes_the_edge_it_comes_over() {
    let mut zones = one();
    assert!(zones.at(110.0, 250.0));
    assert!(!zones.at(112.0, 250.0));
    assert!(zones.at(50.0, 250.0));
    assert_eq!(zones.target(), None);
    zones.at(400.0, 60.0);
    assert_eq!(zones.target(), Some((1, Side::Up)));
}

#[test]
fn a_landing_is_worked_out_once_for_each_pane_and_side() {
    let mut zones = two();
    let made = AtomicU32::new(0);
    let make = |pane: u8, side: Side| {
        made.fetch_add(1, Ordering::SeqCst);
        Some(u32::from(pane) * 10 + u32::from(side.forward()))
    };
    zones.at(10.0, 200.0);
    assert_eq!(zones.plan(make).copied(), Some(10));
    assert_eq!(zones.plan(make).copied(), Some(10));
    zones.at(590.0, 200.0);
    assert_eq!(zones.plan(make).copied(), Some(21));
    zones.at(10.0, 200.0);
    assert_eq!(zones.plan(make).copied(), Some(10));
    assert_eq!(made.load(Ordering::SeqCst), 2);
    // None where nothing is taken; a landing that changes nothing is kept as none.
    zones.at(-5.0, 200.0);
    assert_eq!(zones.plan(make), None);
    let mut zones = one();
    zones.at(110.0, 250.0);
    assert_eq!(zones.plan(|_, _| None::<u32>), None);
    assert_eq!(zones.plan(|_, _| Some(1)), None);
}
