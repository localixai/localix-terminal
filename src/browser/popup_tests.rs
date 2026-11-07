//! Tests of `popup`.

use super::*;

#[test]
fn a_place_given_back_is_not_counted_against_the_tabs_the_app_may_open() {
    begin();
    let first = reserve().unwrap();
    let second = reserve().unwrap();
    // The newest place given back is simply not taken; an older one is left for a popup.
    release(second);
    assert_eq!(reserve(), Some(second));
    release(first);
    assert_eq!(with(|places| places.free.clone()), vec![first]);
    end();
}

#[test]
fn the_places_a_closed_window_left_are_given_to_the_next_tabs_before_new_ones() {
    begin();
    let ids: Vec<u32> = (0..MAX_TABS).filter_map(|_| reserve()).collect();
    assert_eq!(ids.len(), MAX_TABS);
    assert_eq!(reserve(), None);
    // Pages of a closed window: their places are the app's again, the smallest first.
    free(ids[3]);
    free(ids[2]);
    assert_eq!(reserve(), Some(ids[2]));
    assert_eq!(reserve(), Some(ids[3]));
    assert_eq!(reserve(), None);
    end();
}
