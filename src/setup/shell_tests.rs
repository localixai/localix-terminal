//! Tests of `shell`: the person's login shell as the app runs it, and the PATH it reads.

use super::{MARK, Ran, failure, on_path, path_in, private_file, within, within_given};
use std::path::Path;
use std::time::Duration;

#[test]
fn a_shell_that_leaves_a_helper_behind_still_answers_at_once() {
    let started = std::time::Instant::now();
    let mut shell = std::process::Command::new("/bin/sh");
    shell.args(["-c", "sleep 30 & printf hi"]);
    let ran = within(shell, Duration::from_secs(5)).unwrap();
    assert!(ran.ok);
    assert_eq!(ran.out, "hi");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    // One that does not end in time is stopped, and says nothing.
    let mut slow = std::process::Command::new("/bin/sh");
    slow.args(["-c", "sleep 30"]);
    assert!(within(slow, Duration::from_millis(200)).is_err());
    // One that fails says so.
    let mut failing = std::process::Command::new("/bin/sh");
    failing.args(["-c", "echo no; exit 3"]);
    assert!(!within(failing, Duration::from_secs(5)).unwrap().ok);
}

#[test]
fn the_path_is_what_is_between_the_marks_and_never_what_else_the_shell_said() {
    let said = format!(
        "bash: cannot set terminal process group\nwelcome back\n\n{MARK}/usr/bin:/bin{MARK}\n"
    );
    assert_eq!(path_in(&said).as_deref(), Some("/usr/bin:/bin"));
    // No marks, an empty PATH: none.
    assert_eq!(path_in("/usr/bin:/bin"), None);
    assert_eq!(path_in(&format!("{MARK}{MARK}")), None);
}

#[test]
fn a_program_that_cannot_run_says_so_and_a_failure_with_no_word_says_what_it_means() {
    let missing = std::process::Command::new("/nonexistent/llx-desk-program");
    let why = within(missing, Duration::from_secs(1)).err().unwrap();
    assert!(why.contains("could not be started"), "{why}");
    let mut slow = std::process::Command::new("/bin/sh");
    slow.args(["-c", "sleep 30"]);
    let why = within(slow, Duration::from_millis(100)).err().unwrap();
    assert!(why.contains("did not end within"), "{why}");
    let quiet = Ran {
        ok: false,
        out: " \n".into(),
    };
    assert_eq!(failure(&quiet, "it failed"), "it failed");
    let said = Ran {
        ok: false,
        out: "no plugin\n".into(),
    };
    assert_eq!(failure(&said, "it failed"), "no plugin");
}

#[test]
fn what_a_program_is_given_is_read_back_from_a_file_of_its_own() {
    let mut cat = std::process::Command::new("/bin/cat");
    cat.arg("-");
    let ran = within_given(cat, Duration::from_secs(5), Some("an answer")).unwrap();
    assert_eq!(ran.out, "an answer");
    // Two files of one run never share a name, and none is left behind.
    let (one, _) = private_file("x").unwrap();
    let (other, _) = private_file("x").unwrap();
    assert_ne!(one, other);
    let _ = std::fs::remove_file(one);
    let _ = std::fs::remove_file(other);
}

#[test]
fn a_directory_is_on_a_path_by_its_whole_name() {
    assert!(on_path(
        "/usr/bin:/Users/me/.local/bin",
        Path::new("/Users/me/.local/bin")
    ));
    assert!(!on_path("/usr/bin", Path::new("/Users/me/.local/bin")));
}
