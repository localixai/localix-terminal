//! Tests of `catalog::source::registry::pass`: the first pass, a pass cut short, the passes
//! after.

use super::*;

/// 2026-11-12T13:18:20Z.
const NOW: u64 = 1_794_489_500;

#[test]
fn the_first_pass_reads_everything_and_goes_on_where_it_was_cut_short() {
    let first = Pass::after(None, NOW);
    assert_eq!(first.since, None);
    assert_eq!(first.cursor, None);
    assert_eq!(first.began, "2026-11-12T13:08:20Z");
    let cut = first.read(Some("ac.snag/snag:1.0.0".into()));
    assert!(!cut.done);
    // Read again an hour later, it is the same pass, on from its page.
    let again = Pass::after(Some(&cut.mark()), NOW + 3600);
    assert_eq!(again, cut);
}

#[test]
fn a_pass_after_one_done_asks_for_what_was_updated_since_that_one_began() {
    let done = Pass::after(None, NOW).read(None);
    assert!(done.done);
    let next = Pass::after(Some(&done.mark()), NOW + 7200);
    assert_eq!(next.since.as_deref(), Some("2026-11-12T13:08:20Z"));
    assert_eq!(next.began, "2026-11-12T15:08:20Z");
    assert_eq!((next.cursor, next.done), (None, false));
}

#[test]
fn a_mark_that_does_not_read_starts_over() {
    assert_eq!(Pass::after(Some("e7938c50"), NOW), Pass::after(None, NOW));
}

#[test]
fn a_time_is_written_as_rfc_3339() {
    assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339(1_709_251_199), "2024-02-29T23:59:59Z");
    assert_eq!(rfc3339(4_102_444_800), "2100-01-01T00:00:00Z");
}
