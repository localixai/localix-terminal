//! Tests of `platform_bounds`.

use super::*;

const ASKED: Spot = [0.0, 52.0, 800.5, 600.0];
/// Where the system put it: whole points.
const STOOD: Spot = [0.0, 52.0, 801.0, 600.0];

#[test]
fn a_view_asked_for_the_place_it_was_given_and_still_holds_is_left_alone() {
    let last = Placed {
        asked: ASKED,
        stood: STOOD,
    };
    assert!(stands(Some(&last), ASKED, STOOD));
}

#[test]
fn a_view_never_placed_or_asked_for_another_place_is_placed() {
    let last = Placed {
        asked: ASKED,
        stood: STOOD,
    };
    assert!(!stands(None, ASKED, STOOD));
    assert!(!stands(Some(&last), [0.0, 52.0, 900.0, 600.0], STOOD));
}

#[test]
fn a_view_the_window_moved_since_is_placed_again() {
    // Resized with its window (its autoresizing), or another view at the same address.
    let last = Placed {
        asked: ASKED,
        stood: STOOD,
    };
    assert!(!stands(Some(&last), ASKED, [0.0, 52.0, 1001.0, 600.0]));
}
