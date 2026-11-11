//! Regression tests for a real terminal's environment, independent of its automation caller.

use crate::terminal::{DEFAULT_WAIT, Session, Until};

const CAPABILITIES: &str = r#"printf '%s\n' \
    "term=$TERM" "depth=$COLORTERM" "program=$TERM_PROGRAM" "version=$TERM_PROGRAM_VERSION" \
    "no_color=${NO_COLOR-unset}" "force_color=${FORCE_COLOR-unset}" \
    "clicolor=$CLICOLOR" "clicolor_force=${CLICOLOR_FORCE-unset}" \
    "kept=${DESK_COLOR_TEST-unset}"; test -t 0 && test -t 1 && test -t 2 && printf 'tty=all\n'"#;

fn run(command: &str, vars: Option<&[(String, String)]>) -> String {
    let session = Session::start(command, &std::env::temp_dir(), vars, None).unwrap();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Exited);
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
            "term=xterm-256color\ndepth=truecolor\nprogram=llx-desk\nversion={}\n\
             no_color=unset\nforce_color=unset\nclicolor=1\nclicolor_force=unset\nkept=kept\ntty=all",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn an_inherited_server_environment_also_gets_the_terminals_capabilities() {
    let text = run(CAPABILITIES, None);
    for line in [
        "program=llx-desk",
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
