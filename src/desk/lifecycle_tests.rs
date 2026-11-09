//! Tests of `lifecycle`: the question a quit asks while terminals run.

use super::quit_detail;

#[test]
fn the_question_counts_what_quitting_ends() {
    assert!(quit_detail(1).starts_with("A terminal is still running"));
    assert!(quit_detail(3).starts_with("3 terminals are still running"));
}
