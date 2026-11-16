//! Tests of `wait`: a text found in a flood, and a program that asks for a line it hides. And of
//! what a program is typed: lines as one paste where it takes pastes.

use std::time::Duration;

use super::super::{DEFAULT_WAIT, Session, Until};

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

#[test]
fn a_text_at_the_end_of_a_flood_is_found() {
    let session = start("sh -c 'yes flood | head -n 200000; echo the-end-of-it; sleep 20'");
    assert_eq!(
        session.wait(Some("the-end-of-it"), 0, DEFAULT_WAIT),
        Until::Matched
    );
}

#[test]
fn a_program_that_asks_for_a_secret_is_told_of_not_waited_out() {
    let session = start(
        "sh -c 'stty -echo; printf \"Password: \"; read p; stty echo; echo got-it; sleep 20'",
    );
    assert_eq!(session.wait(Some("got-it"), 0, DEFAULT_WAIT), Until::Asking);
    let screen = session.snapshot();
    assert!(screen.contains("asking for hidden input"), "{screen}");
    session.type_in("secret\n", &[]).unwrap();
    assert_eq!(
        session.wait(Some("got-it"), 0, DEFAULT_WAIT),
        Until::Matched
    );
    assert!(!session.snapshot().contains("hidden input"));
}

#[test]
fn a_full_screen_program_that_shows_no_typing_is_not_asking() {
    let session = start("sh -c 'stty raw -echo; printf ready; sleep 20'");
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    assert_eq!(
        session.wait(Some("never"), 0, Duration::from_millis(1200)),
        Until::Deadline
    );
    assert!(!session.snapshot().contains("hidden input"));
}

#[test]
fn lines_reach_a_program_that_takes_pastes_as_one_paste() {
    let session = start("sh -c 'stty raw -echo; printf \"\\033[?2004hready\"; cat -v'");
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    session.type_in("a\nb\n", &[]).unwrap();
    let wanted = "^[[200~a^Mb^[[201~^M";
    assert_eq!(
        session.wait(Some(wanted), 0, DEFAULT_WAIT),
        Until::Matched,
        "{}",
        session.snapshot()
    );
}

/// A program that says, as a shell does, that a command started and ended with code 3.
const MARKED: &str =
    "sh -c 'printf \"\\033]133;C\\007\"; sleep 0.2; printf \"out\\033]133;D;3\\007\"; sleep 20'";

#[test]
fn a_command_the_shell_says_ended_ends_the_wait_with_its_code() {
    let session = start(MARKED);
    let Until::Finished(ended) = session.wait(None, 0, DEFAULT_WAIT) else {
        panic!("{}", session.snapshot());
    };
    assert_eq!(ended.code, Some(3));
    assert!(ended.took >= Duration::from_millis(150), "{:?}", ended.took);
    let said = Until::Finished(ended).said(None, DEFAULT_WAIT);
    assert!(
        said.starts_with("The command ended with exit code 3 after 0."),
        "{said}"
    );
    assert!(
        session
            .snapshot()
            .ends_with("(the shell's last command exited with 3)")
    );
    // It is told once: the next wait is on what comes after.
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}

#[test]
fn a_text_that_never_shows_is_not_waited_for_past_the_commands_end() {
    let session = start(MARKED);
    let until = session.wait(Some("never"), 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    let said = until.said(Some("never"), DEFAULT_WAIT);
    assert!(said.ends_with("; \"never\" did not show."), "{said}");
    // The text that did show is what the wait ends on.
    let session = start(MARKED);
    assert_eq!(session.wait(Some("out"), 0, DEFAULT_WAIT), Until::Matched);
}

/// As a shell marks them: a command's start, and its end with `code`.
const START: &str = "printf \"\\033]133;C\\007\"";
fn end(code: u8) -> String {
    format!("printf \"\\033]133;D;{code}\\007\"")
}

#[test]
fn a_command_that_goes_quiet_is_waited_for_to_its_end() {
    // Quiet for longer than settles an unmarked terminal, then over.
    let session = start(&format!(
        "sh -c '{START}; echo building; sleep 1.8; {}; sleep 20'",
        end(4)
    ));
    let Until::Finished(ended) = session.wait(None, 0, DEFAULT_WAIT) else {
        panic!("{}", session.snapshot());
    };
    assert_eq!(ended.code, Some(4));
}

#[test]
fn a_command_that_takes_keys_settles_as_ever() {
    // A full-screen program or an editor's line: it waits for the person, not for its end.
    let session = start(&format!(
        "sh -c '{START}; stty raw -echo; printf ready; sleep 20'"
    ));
    let begun = std::time::Instant::now();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
    assert!(
        begun.elapsed() < Duration::from_secs(2),
        "{:?}",
        begun.elapsed()
    );
}

#[test]
fn a_command_still_running_when_the_time_is_out_is_said_to_be() {
    let session = start(&format!(
        "sh -c '{START}; while :; do printf .; sleep 0.1; done'"
    ));
    let most = Duration::from_millis(2200);
    assert_eq!(session.wait(None, 0, most), Until::Running);
    let said = Until::Running.said(None, most);
    assert_eq!(
        said,
        "The command is still running after 2s: wait again, or read the screen."
    );
}

#[test]
fn the_end_waited_for_is_of_the_command_typed_not_of_the_one_before_it() {
    // A line typed while a command runs is taken by the shell the moment that one ends.
    let session = start(&format!(
        "sh -c '{START}; sleep 0.3; {}; {START}; sleep 0.3; {}; sleep 20'",
        end(1),
        end(0)
    ));
    let Until::Finished(ended) = session.wait(None, 0, DEFAULT_WAIT) else {
        panic!("{}", session.snapshot());
    };
    assert_eq!(ended.code, Some(0));
}

/// As a shell says its prompt is ready.
const PROMPT: &str = "printf \"\\033]133;A\\007\"";

#[test]
fn a_line_is_typed_only_into_the_shell_in_front_at_a_prompt_nothing_was_typed_at() {
    let session = start(&format!(
        "sh -c '{PROMPT}; printf ready; read line; {START}; printf busy; sleep 1; {}; {PROMPT}; printf again; sleep 20'",
        end(0)
    ));
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    assert!(session.at_prompt(true), "{}", session.snapshot());
    // The person typed at it: what is on the line now is theirs.
    session.typed("a");
    assert!(!session.at_prompt(true) && !session.at_prompt(false));
    // A reply of the terminal's own (its attributes, a report of the pointer) is no typing.
    let fresh = start(&format!("sh -c '{PROMPT}; printf ready; sleep 20'"));
    assert_eq!(fresh.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    fresh.typed("\x1b[?1;2c");
    assert!(fresh.at_prompt(true));
    // An arrow brings a line back from the history: typing.
    fresh.typed("\x1b[A");
    assert!(!fresh.at_prompt(true));
    // A command at work is never typed into.
    session.typed("\r");
    assert_eq!(session.wait(Some("busy"), 0, DEFAULT_WAIT), Until::Matched);
    assert!(!session.at_prompt(false), "{}", session.snapshot());
    // Its end and a new prompt: clean again.
    assert_eq!(session.wait(Some("again"), 0, DEFAULT_WAIT), Until::Matched);
    assert!(session.at_prompt(true), "{}", session.snapshot());
}

#[test]
fn a_shell_that_never_marks_a_prompt_takes_a_line_only_when_one_is_asked_for() {
    let session = start("sh -c 'printf ready; sleep 20'");
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    assert!(!session.at_prompt(true));
    assert!(session.at_prompt(false));
}

#[test]
fn a_program_in_front_of_the_shell_takes_no_line_of_the_apps() {
    // With job control the shell puts its command in a group of its own, in front.
    let session = start(&format!(
        "sh -c 'set -m; {PROMPT}; printf ready; sleep 30; sleep 20'"
    ));
    assert_eq!(session.wait(Some("ready"), 0, DEFAULT_WAIT), Until::Matched);
    let front = std::time::Instant::now() + Duration::from_secs(5);
    while session.foreground().as_deref() != Some("sleep") && std::time::Instant::now() < front {
        crate::backoff(Duration::from_millis(50));
    }
    assert_eq!(session.foreground().as_deref(), Some("sleep"));
    assert!(!session.at_prompt(false) && !session.at_prompt(true));
}
