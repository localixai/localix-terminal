//! Tests of `shell`: the person's login shell as the app runs it, and the PATH and the agents'
//! variables it reads.

use std::collections::BTreeMap;
use std::time::Duration;

use super::{MARK, Ran, VAR_MARK, failure, path_in, private_file, run, vars_in, which, within};

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
    let missing = std::process::Command::new("/nonexistent/llxterm-program");
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
fn a_program_is_found_on_the_path_given_and_run_with_the_variables_given() {
    let sh = which("sh", "/nonexistent:/bin").unwrap();
    assert_eq!(sh, std::path::Path::new("/bin/sh"));
    assert_eq!(which("sh", "/nonexistent"), None);
    let vars = BTreeMap::from([("CODEX_HOME".to_string(), "/work/codex".to_string())]);
    let ran = run(
        &sh,
        &["-c", "printf '%s %s' \"$CODEX_HOME\" \"$PATH\""],
        "/bin",
        &vars,
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(ran.ok);
    assert_eq!(ran.out, "/work/codex /bin");
    // Two files of one run never share a name, and none is left behind.
    let (one, _) = private_file().unwrap();
    let (other, _) = private_file().unwrap();
    assert_ne!(one, other);
    let _ = std::fs::remove_file(one);
    let _ = std::fs::remove_file(other);
}

#[test]
fn the_agents_variables_are_the_ones_the_shell_has_set_between_their_marks() {
    let said = format!(
        "welcome\n{MARK}/usr/bin{MARK}\n{VAR_MARK}CODEX_HOME=/work/codex home{VAR_MARK}\n\
         {VAR_MARK}GEMINI_CLI_HOME={VAR_MARK}\n{VAR_MARK}XDG_CONFIG_HOME=/a=b{VAR_MARK}\r\n\
         {VAR_MARK}SECRET=1{VAR_MARK}\nCOPILOT_HOME=/not/marked\n"
    );
    // One not set says nothing, one that is no agent's home is not kept, and a value keeps
    // its spaces and its own `=`.
    assert_eq!(
        vars_in(&said),
        [
            ("CODEX_HOME".to_string(), "/work/codex home".to_string()),
            ("XDG_CONFIG_HOME".to_string(), "/a=b".to_string()),
        ]
    );
    assert!(vars_in("").is_empty());
}

#[test]
fn a_real_shell_prints_each_variable_on_its_own_line() {
    // As the login shell is asked: `printf` goes through its format once for each pair.
    let mut shell = std::process::Command::new("/bin/sh");
    shell.env("CODEX_HOME", "/work/c x").env_remove("COPILOT_HOME").args([
        "-c",
        &format!("printf '{VAR_MARK}%s=%s{VAR_MARK}\\n' CODEX_HOME \"$CODEX_HOME\" COPILOT_HOME \"$COPILOT_HOME\""),
    ]);
    let ran = within(shell, Duration::from_secs(5)).unwrap();
    assert_eq!(
        vars_in(&ran.out),
        [("CODEX_HOME".to_string(), "/work/c x".to_string())]
    );
}
