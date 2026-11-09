//! Tests of `host_tear`: what a carried tab tells the window, and how it is read.

use super::{plain, said};

#[test]
fn a_carried_tab_is_told_by_its_screen_point_and_its_window_reads_its_own_point_only() {
    assert_eq!(
        said("drop-over w1 -30 12 400 220"),
        Some(("drop-over", "w1", (-30.0, 12.0), Some((400.0, 220.0))))
    );
    assert_eq!(said("drop t2 5 6"), Some(("drop", "t2", (5.0, 6.0), None)));
    assert_eq!(said("drop-over w1 x 1"), None);
    assert_eq!(plain("drop-over w1 -30 12 400 220"), "drop-over w1 -30 12");
    assert_eq!(plain("drop t2 5 6 7 8"), "drop t2 5 6");
    assert_eq!(plain("move t2 end"), "move t2 end");
}
