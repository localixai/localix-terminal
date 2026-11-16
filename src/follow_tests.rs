//! Tests of `follow`: what the loop wakes on, and that it looks for itself only where it
//! cannot be told.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use super::{Heard, RETRY, Waits, parent};

/// A loop's waits over a channel of its own, with what can be waited on.
fn waits(parent_waited: bool, folder_waited: bool) -> (Sender<Heard>, Waits) {
    #[expect(
        clippy::disallowed_methods,
        reason = "the loop's own channel, as `run` makes it"
    )]
    let (tell, heard) = std::sync::mpsc::channel();
    let waits = Waits {
        heard,
        parent: parent(),
        parent_waited,
        folder_waited,
    };
    (tell, waits)
}

#[test]
fn what_is_told_wakes_the_loop_and_nothing_else_does() {
    let (tell, waits) = waits(true, true);
    tell.send(Heard::Appeared).unwrap();
    assert!(matches!(waits.next(false), Some(Heard::Appeared)));
    tell.send(Heard::Orphaned).unwrap();
    assert!(matches!(waits.next(true), Some(Heard::Orphaned)));
    // Nothing told: the loop sleeps (here, until the channel ends).
    drop(tell);
    assert!(waits.next(true).is_none());
}

#[test]
fn where_the_folder_cannot_be_watched_the_app_is_looked_for_after_a_while() {
    let (_tell, waits) = waits(true, false);
    let began = Instant::now();
    assert!(matches!(waits.next(false), Some(Heard::Appeared)));
    assert!(began.elapsed() + Duration::from_millis(50) >= RETRY);
}
