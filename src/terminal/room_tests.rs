//! Tests of `room`: how many terminals the app holds, by each of the machine's limits.

use super::{FLOOR, bound, most};

const GIB: u64 = 1 << 30;

#[test]
fn the_open_file_limit_bounds_terminals_by_the_descriptors_each_holds() {
    // An app the system started with 256 that could not raise it: half of it is kept for the
    // rest of the app, and a terminal holds three.
    assert_eq!(bound(Some(256), None), 42);
    assert_eq!(bound(Some(10_240), None), 1706);
}

#[test]
fn memory_bounds_terminals_by_their_pages() {
    // A quarter of the machine's memory, at 48 MiB a terminal's page.
    assert_eq!(bound(None, Some(24 * GIB)), 128);
    assert_eq!(bound(Some(10_240), Some(8 * GIB)), 42);
}

#[test]
fn the_tighter_limit_holds_and_never_under_what_the_app_always_held() {
    assert_eq!(bound(Some(256), Some(64 * GIB)), 42);
    assert_eq!(bound(Some(64), Some(GIB)), FLOOR);
    assert_eq!(bound(None, None), FLOOR);
}

#[test]
fn this_machine_is_read() {
    assert!(most() >= FLOOR);
}
