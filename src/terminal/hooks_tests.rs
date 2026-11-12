//! Tests of the shell's hooks, against the zsh and the bash of this machine where there is one:
//! its commands are said to end, and the person's own files are read as ever.

use std::path::{Path, PathBuf};

use super::super::{DEFAULT_WAIT, Session, Until};

/// A home folder of a test's own, gone with it.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let home =
            std::env::temp_dir().join(format!("llxterm-shell-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        Self(home)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A zsh of the person's in `home`, whose `.zshrc` (in `zdotdir`, when it has one) names its
/// prompt `ready> `; `None` where there is no zsh.
fn zsh(home: &Path, zdotdir: Option<&str>, more: &[(&str, &str)]) -> Option<Session> {
    let shell = ["/bin/zsh", "/usr/bin/zsh"]
        .into_iter()
        .find(|shell| Path::new(shell).exists())?;
    super::keep_in(std::env::temp_dir().join("llxterm-test-shell"));
    let rc = zdotdir.map_or_else(|| home.to_path_buf(), |own| home.join(own));
    std::fs::create_dir_all(&rc).unwrap();
    std::fs::write(rc.join(".zshrc"), "PS1='ready> '\nFROM_RC=yes\n").unwrap();
    let mut vars = vec![
        ("SHELL".to_string(), shell.to_string()),
        ("HOME".to_string(), home.to_string_lossy().into_owned()),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
    ];
    if zdotdir.is_some() {
        vars.push(("ZDOTDIR".into(), rc.to_string_lossy().into_owned()));
    }
    vars.extend(
        more.iter()
            .map(|(name, value)| ((*name).into(), (*value).into())),
    );
    let session = Session::start("", home, Some(&vars), None).unwrap();
    assert_eq!(
        session.wait(Some("ready>"), 0, DEFAULT_WAIT),
        Until::Matched,
        "{}",
        session.snapshot()
    );
    Some(session)
}

#[test]
fn the_persons_zsh_says_how_a_command_ended_and_reads_their_files_as_ever() {
    let home = Home::new("ends");
    let Some(session) = zsh(&home.0, None, &[]) else {
        return;
    };
    session
        .type_in("echo rc=$FROM_RC zdot=${ZDOTDIR-none}; (exit 7)\n", &[])
        .unwrap();
    let Until::Finished(ended) = session.wait(None, 0, DEFAULT_WAIT) else {
        panic!("{}", session.snapshot());
    };
    assert_eq!(ended.code, Some(7));
    let screen = session.snapshot();
    assert!(screen.contains("rc=yes zdot=none"), "{screen}");
    // Enter alone runs nothing, and nothing is said to have ended.
    session.type_in("\n", &[]).unwrap();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}

#[test]
fn a_zsh_with_a_folder_of_its_own_keeps_it() {
    let home = Home::new("own");
    let Some(session) = zsh(&home.0, Some("own"), &[]) else {
        return;
    };
    session
        .type_in("echo zdot=${ZDOTDIR:t} kept=${LLXTERM_ZDOTDIR-no}\n", &[])
        .unwrap();
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    let screen = session.snapshot();
    assert!(screen.contains("zdot=own kept=no"), "{screen}");
}

#[test]
fn a_shell_told_not_to_is_left_unmarked() {
    let home = Home::new("off");
    let Some(session) = zsh(&home.0, None, &[("LLXTERM_SHELL_MARKS", "0")]) else {
        return;
    };
    session.type_in("echo done\n", &[]).unwrap();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}

/// A bash of the person's in `home`, whose `.bash_profile` names its prompt `ready> ` and has
/// a command of its own run at each prompt; `None` where there is no bash.
fn bash(home: &Path) -> Option<Session> {
    bash_with(
        home,
        "PS1='ready> '\nFROM_PROFILE=yes\nPROMPT_COMMAND='AT_PROMPT=yes'\n",
    )
}

/// A bash of the person's in `home` whose `.bash_profile` is `profile`, at its first prompt.
fn bash_with(home: &Path, profile: &str) -> Option<Session> {
    let shell = "/bin/bash";
    if !Path::new(shell).exists() {
        return None;
    }
    super::keep_in(std::env::temp_dir().join("llxterm-test-shell"));
    std::fs::write(home.join(".bash_profile"), profile).unwrap();
    let vars = vec![
        ("SHELL".to_string(), shell.to_string()),
        ("HOME".to_string(), home.to_string_lossy().into_owned()),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
    ];
    let session = Session::start("", home, Some(&vars), None).unwrap();
    assert_eq!(
        session.wait(Some("ready>"), 0, DEFAULT_WAIT),
        Until::Matched,
        "{}",
        session.snapshot()
    );
    Some(session)
}

#[test]
fn the_persons_bash_says_how_a_command_ended_and_reads_their_files_as_ever() {
    let home = Home::new("bash-ends");
    let Some(session) = bash(&home.0) else {
        return;
    };
    session
        .type_in(
            "echo profile=$FROM_PROFILE prompt=$AT_PROMPT; (exit 7)\n",
            &[],
        )
        .unwrap();
    let Until::Finished(ended) = session.wait(None, 0, DEFAULT_WAIT) else {
        panic!("{}", session.snapshot());
    };
    assert_eq!(ended.code, Some(7));
    let screen = session.snapshot();
    assert!(screen.contains("profile=yes prompt=yes"), "{screen}");
    // Enter alone runs nothing, and nothing is said to have ended.
    session.type_in("\n", &[]).unwrap();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}

#[test]
fn a_bash_keeps_its_last_word_and_its_last_code() {
    // What the hooks run between two commands is not seen by the second.
    let home = Home::new("bash-keeps");
    let Some(session) = bash(&home.0) else {
        return;
    };
    session
        .type_in("echo one two; echo word=$_\n", &[])
        .unwrap();
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    session.type_in("false\n", &[]).unwrap();
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    session.type_in("echo code=$?\n", &[]).unwrap();
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    let screen = session.snapshot();
    assert!(
        screen.contains("word=two") && screen.contains("code=1"),
        "{screen}"
    );
}

#[test]
fn a_bash_whose_own_prompt_command_ends_in_a_semicolon_or_is_strict_keeps_its_prompt() {
    // Two things a profile commonly has: a prompt command closed with `;`, and `set -u`, under
    // which a name read before it is set is an error.
    for (name, profile) in [
        (
            "bash-semicolon",
            "PS1='ready> '\nPROMPT_COMMAND='AT_PROMPT=yes;'\n",
        ),
        ("bash-strict", "PS1='ready> '\nset -u\n"),
    ] {
        let home = Home::new(name);
        let Some(session) = bash_with(&home.0, profile) else {
            return;
        };
        session.type_in("sh -c 'exit 3'\n", &[]).unwrap();
        let until = session.wait(None, 0, DEFAULT_WAIT);
        let screen = session.snapshot();
        let Until::Finished(ended) = until else {
            panic!("{name}: {until:?}\n{screen}");
        };
        assert_eq!(ended.code, Some(3), "{name}");
        assert!(
            !screen.contains("syntax error") && !screen.contains("unbound variable"),
            "{name}: {screen}"
        );
    }
}
