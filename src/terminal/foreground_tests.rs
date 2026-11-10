//! Tests of `foreground`.

use super::*;

fn args(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_string).collect()
}

#[test]
fn a_program_is_known_by_the_name_it_was_started_as() {
    assert_eq!(named(&args("claude --resume")).as_deref(), Some("claude"));
    assert_eq!(
        named(&args("/usr/local/bin/codex")).as_deref(),
        Some("codex")
    );
    // A login shell's dash is no part of its name.
    assert_eq!(named(&args("-zsh")).as_deref(), Some("zsh"));
}

#[test]
fn a_script_is_known_by_the_script_not_its_runtime() {
    let line = args("node --no-warnings /opt/homebrew/bin/claude -p hi");
    assert_eq!(named(&line).as_deref(), Some("claude"));
    assert_eq!(named(&args("node")).as_deref(), Some("node"));
    assert_eq!(named(&[]), None);
}

#[test]
fn this_process_has_arguments() {
    let pid = libc::pid_t::try_from(std::process::id()).unwrap();
    let read = arguments(pid).expect("this process's arguments");
    assert!(!read.is_empty());
}

#[test]
fn a_shell_that_only_runs_a_command_is_looked_past() {
    assert!(runs_a_command(&args("sh -c claude")));
    assert!(runs_a_command(&args("/bin/dash -c x")));
    // A person's shell, or a shell running a script, is the program in front.
    assert!(!runs_a_command(&args("-zsh")));
    assert!(!runs_a_command(&args("bash script.sh")));
    assert!(!runs_a_command(&args("node -c x")));
}
