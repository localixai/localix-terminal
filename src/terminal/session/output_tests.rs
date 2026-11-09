//! Tests of `output`: a program's cursor queries found in its output, read by read.

use super::Queries;

#[test]
fn a_cursor_query_split_across_two_reads_is_answered_once() {
    let mut queries = Queries::default();
    assert_eq!(queries.asked(b"prompt\x1b["), 0);
    assert_eq!(queries.asked(b"6n more"), 1);
    // Byte by byte, then two in one read: each found once.
    let asked: usize = (b"\x1b[6n".iter())
        .map(|byte| queries.asked(&[*byte]))
        .sum();
    assert_eq!(asked, 1);
    assert_eq!(queries.asked(b"\x1b[6n\x1b[6n"), 2);
    assert_eq!(queries.asked(b"6n"), 0, "the end of one is no new one");
}
