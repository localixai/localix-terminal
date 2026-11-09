//! Tests of `masks`.

use super::*;

/// The numbers are AppKit's: a name that drifted from its value would send another bit.
#[test]
fn each_name_holds_appkits_number() {
    assert_eq!((BORDERLESS, NONACTIVATING_PANEL), (0, 0x80));
    assert_eq!(
        CAN_JOIN_ALL_SPACES | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY,
        0x141
    );
    assert_eq!(FULL_SCREEN_PRIMARY, 0x80);
    assert_eq!(WIDTH_SIZABLE | HEIGHT_SIZABLE, 18);
    assert_eq!(
        MIN_X_MARGIN | MAX_X_MARGIN | MIN_Y_MARGIN | MAX_Y_MARGIN,
        45
    );
    assert_eq!(
        (NORMAL_LEVEL, FLOATING_LEVEL, POP_UP_MENU_LEVEL),
        (0, 3, 101)
    );
    assert_eq!((BELOW, ABOVE), (-1, 1));
}
