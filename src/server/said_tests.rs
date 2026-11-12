//! Tests of `said`.

use super::*;
use log::Log;

#[test]
fn a_warning_is_a_line_with_its_time_and_where_it_was_said() {
    assert_eq!(
        line(
            1_794_500_000,
            "llxterm::desk",
            &format_args!("the window did not open: {}", "no folder")
        ),
        "1794500000 llxterm::desk: the window did not open: no folder"
    );
}

#[test]
fn warnings_and_errors_are_written_and_the_rest_is_not() {
    let at = |level| log::Metadata::builder().level(level).build();
    assert!(Said.enabled(&at(log::Level::Error)));
    assert!(Said.enabled(&at(log::Level::Warn)));
    assert!(!Said.enabled(&at(log::Level::Info)));
    assert!(!Said.enabled(&at(log::Level::Debug)));
}
