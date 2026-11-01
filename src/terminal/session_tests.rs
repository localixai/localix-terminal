//! Tests of `session`.

use std::sync::{Arc, Mutex};

use super::*;

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

#[test]
fn output_settles_and_shows_on_the_screen() {
    let session = start("echo hello; echo world");
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Settled | Until::Exited), "{until:?}");
    let screen = session.snapshot();
    assert!(screen.contains("hello\nworld"), "{screen}");
}

#[test]
fn typing_is_waited_for_by_the_text_it_brings() {
    let session = start("sh -c 'printf \"? \"; read x; echo \"you said $x\"; sleep 20'");
    assert_eq!(session.wait(Some("?"), 0, DEFAULT_WAIT), Until::Matched);
    session.type_in("hi\n", &[]).unwrap();
    assert_eq!(
        session.wait(Some("you said hi"), 0, DEFAULT_WAIT),
        Until::Matched
    );
}

#[test]
fn a_program_that_ends_says_how() {
    let session = start("sh -c 'echo bye; exit 3'");
    assert_eq!(
        session.wait(Some("never shown"), 0, DEFAULT_WAIT),
        Until::Exited
    );
    assert!(session.snapshot().contains("exited with 3"));
}

#[test]
fn a_named_key_that_does_not_exist_is_refused_with_the_names() {
    let session = start("sleep 20");
    let why = session.type_in("", &["Hyper".into()]).unwrap_err();
    assert!(why.contains("Enter"), "{why}");
}

#[test]
fn closing_ends_the_program() {
    let session = start("sleep 60");
    let pid = crate::lock(&session.child).id().cast_signed();
    session.close();
    // SAFETY: signal 0 only asks whether the process exists.
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
}

/// A session whose painter keeps the scripts it is sent.
fn painted(command: &str) -> (Session, Arc<Mutex<Vec<String>>>) {
    let scripts = Arc::new(Mutex::new(vec![]));
    let kept = Arc::clone(&scripts);
    let painter: Painter = Box::new(move |script| {
        kept.lock().unwrap().push(script);
        true
    });
    let session = Session::start(command, &std::env::temp_dir(), None, Some(painter)).unwrap();
    (session, scripts)
}

/// The bytes of every `W("...")` sent so far.
fn output(scripts: &Mutex<Vec<String>>) -> Vec<u8> {
    use base64::Engine;
    let mut bytes = vec![];
    for script in scripts.lock().unwrap().iter() {
        if let Some(b64) = script
            .strip_prefix("W(\"")
            .and_then(|s| s.strip_suffix("\")"))
        {
            bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap(),
            );
        }
    }
    bytes
}

#[test]
fn the_window_gets_the_programs_raw_output_colours_included() {
    let (session, scripts) = painted(r"printf '\033[31mred\033[0m\n'; sleep 20");
    session.wait(Some("red"), 0, DEFAULT_WAIT);
    let deadline = Instant::now() + DEFAULT_WAIT;
    while !String::from_utf8_lossy(&output(&scripts)).contains("\x1b[31mred") {
        assert!(Instant::now() < deadline, "{:?}", scripts.lock().unwrap());
        nap(20);
    }
}

#[test]
fn a_loaded_page_is_sent_the_screen_and_the_terminal_takes_its_size() {
    let (session, scripts) = painted("sh -c 'echo hi; sleep 20'");
    session.wait(Some("hi"), 0, DEFAULT_WAIT);
    session.loaded(100, 30);
    let deadline = Instant::now() + DEFAULT_WAIT;
    while !scripts.lock().unwrap().iter().any(|s| s.starts_with("R(")) {
        assert!(Instant::now() < deadline);
        nap(20);
    }
    assert!(session.snapshot().starts_with("screen 100×30"));
}

#[test]
fn a_resize_reaches_the_program() {
    let session = start("sh -c 'trap \"stty size\" WINCH; while :; do sleep 0.1; done'");
    session.wait(None, 0, Duration::from_millis(500));
    session.resized(90, 25);
    assert_eq!(session.wait(Some("25 90"), 0, DEFAULT_WAIT), Until::Matched);
}

#[test]
fn what_the_person_types_reaches_the_program() {
    let session = start("sh -c 'read x; echo \"got $x\"; sleep 20'");
    session.typed("hello\r");
    assert_eq!(
        session.wait(Some("got hello"), 0, DEFAULT_WAIT),
        Until::Matched
    );
}

#[test]
fn the_program_runs_in_a_256_colour_truecolor_terminal() {
    let session = start("sh -c 'echo $TERM $COLORTERM; sleep 20'");
    assert_eq!(
        session.wait(Some("xterm-256color truecolor"), 0, DEFAULT_WAIT),
        Until::Matched
    );
}
