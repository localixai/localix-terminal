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

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff while the shell starts the program: nothing to wait on"
)]
fn the_program_in_front_is_the_one_the_shell_runs_now() {
    // A shell with job control, as the person's is: what is typed runs in a group of its own, in
    // front of the shell.
    let session = start("sh -c 'set -m; printf \"$ \"; read x; cat'");
    assert_eq!(session.wait(Some("$"), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(session.foreground().as_deref(), Some("sh"));
    let heard = Arc::new(Mutex::new(vec![]));
    let told = Arc::clone(&heard);
    session.on_front(move |program| crate::lock(&told).push(program.map(str::to_string)));
    assert_eq!(*crate::lock(&heard), [Some("sh".to_string())]);
    session.type_in("go\n", &[]).unwrap();
    let deadline = Instant::now() + DEFAULT_WAIT;
    while session.foreground().as_deref() != Some("cat") {
        assert!(Instant::now() < deadline, "{:?}", session.foreground());
        std::thread::sleep(Duration::from_millis(20));
    }
    // What the program in front prints tells whoever listens that it came in front.
    session.type_in("hi\n", &[]).unwrap();
    let deadline = Instant::now() + DEFAULT_WAIT;
    while crate::lock(&heard).last() != Some(&Some("cat".to_string())) {
        assert!(Instant::now() < deadline, "{:?}", crate::lock(&heard));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff while the shell becomes the program: nothing to wait on"
)]
fn a_program_that_replaces_its_shell_is_heard_in_front() {
    // As an agent opened from "+" starts: a shell that becomes the program (`exec`), in the
    // same group, so only its name changes.
    let session = start("sh -c 'printf \"$ \"; read x; exec cat'");
    assert_eq!(session.wait(Some("$"), 0, DEFAULT_WAIT), Until::Matched);
    let heard = Arc::new(Mutex::new(vec![]));
    let told = Arc::clone(&heard);
    session.on_front(move |program| crate::lock(&told).push(program.map(str::to_string)));
    session.type_in("go\n", &[]).unwrap();
    let deadline = Instant::now() + DEFAULT_WAIT;
    while crate::lock(&heard).last() != Some(&Some("cat".to_string())) {
        assert!(Instant::now() < deadline, "{:?}", crate::lock(&heard));
        // Output of the program now in front, as a running agent keeps drawing.
        session.type_in("hi\n", &[]).unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_restored_screen_shows_dimmed_before_the_programs_output() {
    let session = Session::start_after(
        "printf after",
        &std::env::temp_dir(),
        None,
        None,
        "one\ntwo",
    )
    .unwrap();
    let until = session.wait(Some("after"), 0, Duration::from_secs(10));
    assert_ne!(until, Until::Deadline, "{}", session.text());
    assert!(
        session.text().starts_with("one\ntwo\nafter"),
        "{}",
        session.text()
    );
}

#[test]
fn a_bad_key_is_refused_before_any_of_the_input_is_typed() {
    let session = start("cat");
    let why = session
        .type_in("never typed", &["Enter".into(), "Hyper".into()])
        .unwrap_err();
    assert!(why.contains("Hyper"), "{why}");
    session.wait(None, 0, Duration::from_millis(600));
    assert!(
        !session.text().contains("never typed"),
        "{}",
        session.text()
    );
}

#[test]
fn input_and_keys_are_typed_in_order_text_first() {
    let session = start("sh -c 'read x; echo \"got $x\"; sleep 20'");
    session
        .type_in("ab", &["Left".into(), "Backspace".into(), "Enter".into()])
        .unwrap();
    assert_eq!(session.wait(Some("got "), 0, DEFAULT_WAIT), Until::Matched);
}

#[test]
fn a_program_asking_for_the_cursor_is_answered_by_the_session() {
    // Raw, so the reply is not echoed: the program reads its six bytes and shows them.
    let session = start(
        r#"sh -c 'stty raw -echo; printf "\033[6n"; dd bs=1 count=6 2>/dev/null | od -An -c; sleep 20'"#,
    );
    let until = session.wait(Some("1   ;   1   R"), 0, DEFAULT_WAIT);
    assert_eq!(until, Until::Matched, "{}", session.text());
}

#[test]
fn typing_and_resizing_never_wait_for_a_program_that_reads_nothing() {
    let session = start("sh -c 'stty raw -echo; sleep 20'");
    session.wait(None, 0, Duration::from_millis(300));
    let begun = Instant::now();
    session.typed(&"x".repeat(500_000));
    session.resized(90, 25);
    session.typed("more");
    assert!(
        begun.elapsed() < Duration::from_millis(500),
        "{:?}",
        begun.elapsed()
    );
}

#[test]
fn output_still_arriving_is_not_called_settled() {
    let busy = start("sh -c 'while :; do echo more; sleep 0.1; done'");
    assert_eq!(busy.wait(None, 0, DEFAULT_WAIT), Until::Deadline);
    let quiet = start("sleep 20");
    assert_eq!(quiet.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}

#[test]
fn a_text_that_wraps_in_the_pane_is_waited_for_whole() {
    // 150 characters on a 120-column screen: two rows, one line.
    let session = start("sh -c 'printf \"%0150d\" 0; sleep 20'");
    let needle = "0".repeat(150);
    assert_eq!(session.wait(Some(&needle), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(session.count(&needle), 1);
}

#[test]
fn the_screen_is_labelled_as_what_the_program_drew() {
    let session = start("echo hi");
    session.wait(None, 0, DEFAULT_WAIT);
    let screen = session.snapshot();
    assert!(
        screen.starts_with("screen 120×40, cursor at row "),
        "{screen}"
    );
    assert!(screen.contains("(untrusted program output):\n"), "{screen}");
}

#[test]
fn a_program_that_sets_its_title_all_the_time_is_heard_now_and_then_with_the_newest() {
    let session = start(
        r#"sh -c 'sleep 0.4; printf "\033]0;alpha\007"; printf "\033]0;beta\007"; printf "\033]0;gamma\007"; sleep 20'"#,
    );
    let heard = Arc::new(Mutex::new(vec![]));
    let told = Arc::clone(&heard);
    session.on_title(move |title| crate::lock(&told).push(title.to_string()));
    let deadline = Instant::now() + DEFAULT_WAIT;
    while crate::lock(&heard).last().map(String::as_str) != Some("gamma") {
        assert!(Instant::now() < deadline, "{:?}", crate::lock(&heard));
        nap(20);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_terminal_with_no_locale_gets_a_utf8_one_and_one_that_has_one_keeps_it() {
    let path = std::env::var("PATH").unwrap_or_default();
    let run = |extra: &[(&str, &str)]| {
        let mut vars = vec![("PATH".to_string(), path.clone())];
        vars.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        let session = Session::start(
            "sh -c 'echo \"lang=[$LANG]\"; sleep 20'",
            &std::env::temp_dir(),
            Some(&vars),
            None,
        )
        .unwrap();
        session.wait(Some("lang=["), 0, DEFAULT_WAIT);
        session.text()
    };
    assert!(run(&[]).contains("lang=[en_US.UTF-8]"));
    assert!(run(&[("LANG", "C")]).contains("lang=[C]"));
    assert!(run(&[("LC_ALL", "de_DE.UTF-8")]).contains("lang=[]"));
}
