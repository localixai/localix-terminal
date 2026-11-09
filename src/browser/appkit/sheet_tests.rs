//! A question's answer reaches whoever waits for it once, however often the system calls back.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::once;

#[test]
fn an_answer_reaches_its_asker_once() {
    let heard = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&heard);
    let answer = once(move |response: isize| {
        counted.fetch_add(usize::try_from(response).unwrap_or(0), Ordering::Relaxed);
    });
    answer(1000);
    answer(1001);
    assert_eq!(heard.load(Ordering::Relaxed), 1000);
}

#[test]
fn a_sheet_hangs_only_from_a_window_on_screen() {
    assert!(super::hangs(true, false));
    assert!(
        !super::hangs(false, false),
        "a window put away shows no sheet"
    );
    assert!(!super::hangs(true, true), "nor one in the Dock");
}
