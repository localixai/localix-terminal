//! Tests of `nspanel`: how the main window behaves in Spaces and full screen.

use super::super::appkit::masks::{CAN_JOIN_ALL_SPACES, FULL_SCREEN_AUXILIARY};
use super::*;

#[test]
fn deck_goes_full_screen_and_tiles_as_an_ordinary_window() {
    let behavior = DECK_SPACES;
    assert_ne!(behavior & FULL_SCREEN_PRIMARY, 0);
    assert_eq!(behavior & FULL_SCREEN_AUXILIARY, 0);
    assert_eq!(behavior & CAN_JOIN_ALL_SPACES, 0);
}
