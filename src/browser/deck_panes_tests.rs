//! Tests of `deck_panes`: what a divider's page is told of the way it moves.

use super::way;

#[test]
fn a_divider_told_its_way_a_second_time_is_told_in_a_scope_of_its_own() {
    // A view of the pool lies over a divider between panes side by side, then over one between
    // panes one above the other: each telling runs whole, whatever the one before declared.
    for across in [true, false] {
        let script = way(across);
        assert!(
            script.starts_with('{') && script.ends_with('}'),
            "declared in the page's own scope: {script}"
        );
    }
    assert!(way(true).contains("dataset.way='across'"));
    assert!(way(false).contains("dataset.way='down'"));
    assert!(way(false).contains("'aria-orientation','horizontal'"));
}
