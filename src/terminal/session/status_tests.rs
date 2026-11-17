//! Tests of `status`: a program's query answered through its terminal, its reports heard, and
//! its records taken as it ends, as the shell comes back in front, and as the person types.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::super::super::status::Doing;
use super::super::{DEFAULT_WAIT, Session, Until};
use crate::surface::Surface;

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

fn states(session: &Session) -> Vec<(String, Doing)> {
    (session.program_status().into_iter())
        .map(|row| (row.id, row.state))
        .collect()
}

/// Waits until `done` holds, at most `DEFAULT_WAIT`.
#[expect(
    clippy::disallowed_methods,
    reason = "a test's backoff while another thread reads the terminal"
)]
fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + DEFAULT_WAIT;
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_query_is_answered_through_the_terminal_with_or_without_a_page() {
    let session = start(
        r#"sh -c 'stty raw -echo; printf "\033]7501;?\033\\"; dd bs=1 count=9 2>/dev/null | od -An -c; sleep 20'"#,
    );
    let until = session.wait(Some("0   1   ;   ?   :"), 0, DEFAULT_WAIT);
    assert_eq!(until, Until::Matched, "{}", session.snapshot());
}

#[test]
fn a_report_is_read_as_the_terminals_status() {
    let session =
        start(r"printf '\033]7501;state=working:app=claude-code:progress=30\007go'; sleep 20");
    assert_eq!(session.wait(Some("go"), 0, DEFAULT_WAIT), Until::Matched);
    let rows = session.program_status();
    assert_eq!(rows.len(), 1);
    let root = &rows[0];
    assert_eq!(
        (root.state, root.app.as_deref(), root.progress),
        (Doing::Working, Some("claude-code"), Some(30))
    );
}

#[test]
fn what_is_done_stays_until_the_person_types_in_the_terminal() {
    let session = start(r"printf '\033]7501;state=done:app=claude-code\007ready'; sleep 20");
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(states(&session), [(String::new(), Doing::Done)]);
    let heard = Arc::new(AtomicUsize::new(0));
    let told = Arc::clone(&heard);
    session.on_status(move || {
        told.fetch_add(1, Ordering::SeqCst);
    });
    // What the window answers the program for them is not the person; a key of theirs is.
    Surface::text(&session, "\x1b[2;2R");
    assert_eq!(states(&session).len(), 1);
    Surface::text(&session, "y");
    assert!(states(&session).is_empty());
    assert_eq!(heard.load(Ordering::SeqCst), 1);
}

#[test]
fn a_program_that_ends_takes_what_was_at_work_and_leaves_what_failed() {
    let session = start(
        r"printf '\033]7501;state=working\007\033]7501;state=blocked:id=a\007\033]7501;state=error:id=b\007'; sleep 0.3",
    );
    assert_eq!(session.wait(Some("never"), 0, DEFAULT_WAIT), Until::Exited);
    until("the program's end is taken in", || {
        states(&session) == [("b".to_string(), Doing::Error)]
    });
}

#[test]
fn the_shell_back_in_front_takes_what_its_program_left_at_work() {
    // A shell with job control, as the person's is: the program runs in front of it, then ends.
    let session = start(
        r#"sh -c 'set -m; sh -c "printf \"\\033]7501;state=working\\007\\033]7501;state=done:id=a\\007busy \"; sleep 1"; printf back; sleep 20'"#,
    );
    assert_eq!(session.wait(Some("busy"), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(states(&session).len(), 2, "{:?}", states(&session));
    assert_eq!(session.wait(Some("back"), 0, DEFAULT_WAIT), Until::Matched);
    until("what was at work goes with its program", || {
        states(&session) == [("a".to_string(), Doing::Done)]
    });
}

#[test]
fn a_program_that_leaves_the_front_takes_what_it_left_at_work_when_the_next_comes_at_once() {
    // The next job comes in front as the program ends, with no output while the shell was:
    // the shell's own moment in front is never seen, the program's leaving is.
    let session = start(
        r#"sh -c 'set -m; sh -c "printf \"\\033]7501;state=working\\007\\033]7501;state=done:id=a\\007busy \"; sleep 1"; sh -c "sleep 0.5; printf next; sleep 20"'"#,
    );
    assert_eq!(session.wait(Some("busy"), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(states(&session).len(), 2, "{:?}", states(&session));
    assert_eq!(session.wait(Some("next"), 0, DEFAULT_WAIT), Until::Matched);
    until("what was at work goes with its program", || {
        states(&session) == [("a".to_string(), Doing::Done)]
    });
}
