use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use super::{Turn, refused};

#[test]
fn an_action_goes_at_once_while_the_person_is_not_commenting() {
    let turn = Turn::default();
    let told = AtomicUsize::new(0);
    let started = Instant::now();
    assert_eq!(
        turn.wait(Duration::from_secs(5), || {
            told.fetch_add(1, Ordering::SeqCst);
        }),
        Ok(())
    );
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(told.load(Ordering::SeqCst), 0, "no wait, nothing to tell");
}

#[test]
fn an_action_waits_while_the_person_comments_and_goes_when_they_stop() {
    let turn = Arc::new(Turn::default());
    turn.commenting(true);
    let (told, waits) = mpsc::sync_channel(2);
    let (finished, done) = mpsc::sync_channel(1);
    let waiter = Arc::clone(&turn);
    let action = std::thread::Builder::new()
        .name("turn-test".into())
        .spawn(move || {
            let went = waiter.wait(Duration::from_secs(10), || {
                let _ = told.send(());
            });
            let _ = finished.send(went);
        });
    assert!(action.is_ok());
    // Told when the wait starts: the strip says the agent waits.
    assert!(waits.recv_timeout(Duration::from_secs(2)).is_ok());
    assert_eq!(turn.now(), (false, 1), "the action waits for the person");
    assert!(
        done.recv_timeout(Duration::from_millis(100)).is_err(),
        "it does not go while they comment"
    );
    turn.commenting(false);
    assert_eq!(done.recv_timeout(Duration::from_secs(2)).ok(), Some(Ok(())));
    assert_eq!(turn.now(), (false, 0));
}

#[test]
fn an_action_that_waits_too_long_does_not_run_and_says_why() {
    let turn = Turn::default();
    turn.commenting(true);
    let told = AtomicUsize::new(0);
    let waited = turn.wait(Duration::from_millis(120), || {
        told.fetch_add(1, Ordering::SeqCst);
    });
    assert!(
        waited.is_err_and(|waited| waited >= Duration::from_millis(120)),
        "the person still comments: the action must not go"
    );
    assert_eq!(
        told.load(Ordering::SeqCst),
        2,
        "told when it starts and ends waiting"
    );
    assert_eq!(turn.now(), (false, 0));
    assert!(refused(Duration::from_secs(60)).starts_with("the person is commenting"));
    assert!(refused(Duration::from_secs(60)).contains("waited 60 s"));
}

#[test]
fn the_strip_knows_when_the_agent_acts() {
    let turn = Turn::default();
    turn.acting(true);
    assert_eq!(turn.now(), (true, 0));
    turn.acting(false);
    assert_eq!(turn.now(), (false, 0));
}
