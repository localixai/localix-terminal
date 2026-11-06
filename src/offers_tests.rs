//! Tests of `offers`.

use std::sync::Arc;

use super::*;
use crate::browser::Answers;

const DAY: u64 = 24 * 3600;

#[test]
fn an_offer_put_off_waits_longer_each_time() {
    let answer = Answer::default();
    assert!(answer.due(0));
    let answer = answer.later(0);
    assert!(!answer.due(DAY - 1) && answer.due(DAY));
    let answer = answer.later(DAY);
    assert!(!answer.due(4 * DAY - 1) && answer.due(4 * DAY));
    let answer = answer.later(4 * DAY);
    assert!(answer.due(11 * DAY));
    // A month after, however often it was put off since.
    let answer = answer.later(11 * DAY).later(41 * DAY);
    assert!(!answer.due(71 * DAY - 1) && answer.due(71 * DAY));
}

#[test]
fn an_offer_taken_is_never_shown_again_and_the_answers_are_kept() {
    let store = Arc::new(Store::memory().unwrap());
    let kept = Kept(Arc::clone(&store));
    assert!(kept.due("plugin") && kept.due("mcp"));
    kept.answered("plugin", false);
    assert!(!kept.due("plugin"), "put off: not for a day");
    kept.answered("mcp", true);
    assert!(!kept.due("mcp"));
    let mcp = store.read(|db| read(db, "mcp")).unwrap();
    assert!(mcp.taken && !mcp.due(u64::MAX));
    // Another offer is its own.
    assert!(kept.due("other"));
}
