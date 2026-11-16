//! Regression tests for a real terminal's environment, independent of its automation caller.

use crate::terminal::{DEFAULT_WAIT, Session, Until};

const CAPABILITIES: &str = r#"printf '%s\n' \
    "term=$TERM" "depth=$COLORTERM" "program=$TERM_PROGRAM" "version=$TERM_PROGRAM_VERSION" \
    "no_color=${NO_COLOR-unset}" "force_color=${FORCE_COLOR-unset}" \
    "clicolor=$CLICOLOR" "clicolor_force=${CLICOLOR_FORCE-unset}" \
    "kept=${DESK_COLOR_TEST-unset}"; test -t 0 && test -t 1 && test -t 2 && printf 'tty=all\n'"#;

fn run(command: &str, vars: Option<&[(String, String)]>) -> String {
    ended(&Session::start(command, &std::env::temp_dir(), vars, None).unwrap())
}

/// What `session`'s program printed, once it ended. Waited for by text it never prints: a wait
/// with nothing named ends at the first quiet, and a program may be quiet a while before its
/// first output (its executable read for the first time, on a busy machine).
fn ended(session: &Session) -> String {
    let until = session.wait(Some("\u{0}never printed"), 0, DEFAULT_WAIT);
    assert_eq!(until, Until::Exited, "{}", session.snapshot());
    session.text()
}

#[test]
fn an_interactive_terminal_does_not_inherit_automation_color_overrides() {
    let vars = [
        ("PATH", "/usr/bin:/bin"),
        ("TERM", "dumb"),
        ("COLORTERM", "none"),
        ("TERM_PROGRAM", "WarpTerminal"),
        ("TERM_PROGRAM_VERSION", "old"),
        ("NO_COLOR", "1"),
        ("FORCE_COLOR", "0"),
        ("CLICOLOR", "0"),
        ("CLICOLOR_FORCE", "1"),
        ("DESK_COLOR_TEST", "kept"),
    ]
    .map(|(name, value)| (name.to_string(), value.to_string()));
    let text = run(CAPABILITIES, Some(&vars));
    assert_eq!(
        text,
        format!(
            "term=xterm-256color\ndepth=truecolor\nprogram=llxterm\nversion={}\n\
             no_color=unset\nforce_color=unset\nclicolor=1\nclicolor_force=unset\nkept=kept\ntty=all",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn an_inherited_server_environment_also_gets_the_terminals_capabilities() {
    let text = run(CAPABILITIES, None);
    for line in [
        "program=llxterm",
        "no_color=unset",
        "force_color=unset",
        "clicolor=1",
        "clicolor_force=unset",
        "tty=all",
    ] {
        assert!(text.lines().any(|actual| actual == line), "{text}");
    }
}

#[test]
fn a_command_can_explicitly_choose_its_own_color_preference() {
    let text = run(
        r#"NO_COLOR=1 FORCE_COLOR=0 CLICOLOR=0 sh -c 'printf "%s\n" "$NO_COLOR" "$FORCE_COLOR" "$CLICOLOR"'"#,
        None,
    );
    assert_eq!(text, "1\n0\n0");
}

#[test]
fn a_command_the_app_starts_finds_what_the_persons_own_terminal_finds() {
    let person = || Some(std::ffi::OsString::from("/home/me/.local/bin:/usr/bin"));
    // The app's own environment (started from the Finder or the Dock, it has the system's PATH
    // alone): the command is looked for where the person's shell looks.
    assert_eq!(super::command_path(None, person), person());
    // A caller's environment is the caller's: its PATH stays.
    let theirs = [("PATH".to_string(), "/opt/tools".to_string())];
    assert_eq!(super::command_path(Some(&theirs), person), None);
}

/// What `command` prints in a terminal of the app's, told `own`, started with `vars`.
fn run_own(command: &str, vars: &[(&str, &str)], own: &super::super::Own) -> String {
    let vars: Vec<(String, String)> = (vars.iter())
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect();
    ended(
        &Session::start_after(command, &std::env::temp_dir(), Some(&vars), None, "", own).unwrap(),
    )
}

#[test]
fn a_terminal_opened_with_an_agents_environment_says_which_terminal_it_is_itself() {
    // An agent in t1 opens a terminal with its own environment: the new one is t9, not t1.
    let own = super::super::Own {
        bin: None,
        vars: vec![
            ("LLXTERM_PANE".into(), "t9".into()),
            ("LLXTERM_PANE_PROJECT".into(), "/work/shop".into()),
        ],
    };
    let vars = [("PATH", "/usr/bin:/bin"), ("LLXTERM_PANE", "t1")];
    let text = run_own(
        r#"printf '%s %s' "$LLXTERM_PANE" "$LLXTERM_PANE_PROJECT""#,
        &vars,
        &own,
    );
    assert_eq!(text, "t9 /work/shop");
}

#[test]
fn the_apps_command_is_first_on_a_terminals_path() {
    let scratch = crate::testing::Scratch::new();
    let bin = scratch.path().join("bin");
    crate::testing::echo_named(&bin, "llxterm");
    let own = super::super::Own {
        bin: Some(bin.clone()),
        vars: Vec::new(),
    };
    let vars = [("PATH", "/usr/bin:/bin")];
    let text = run_own(
        r#"llxterm ours; printf '%s\n%s' "$PATH" "$LLXTERM_BIN""#,
        &vars,
        &own,
    );
    assert_eq!(
        text,
        format!("ours\n{0}:/usr/bin:/bin\n{0}", bin.display()),
        "the app's command runs, its folder first and named"
    );
}

#[test]
fn the_apps_folder_is_put_first_once() {
    let bin = std::path::Path::new("/app/bin");
    let first = |path: &str| super::super::environment::first_on(bin, Some(&path.into()));
    assert_eq!(first("/usr/bin:/bin"), "/app/bin:/usr/bin:/bin");
    assert_eq!(first("/usr/bin:/app/bin:/bin"), "/app/bin:/usr/bin:/bin");
    assert_eq!(super::super::environment::first_on(bin, None), "/app/bin");
}
