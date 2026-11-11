//! Tests of the shell's hooks, against the zsh of this machine where there is one: its commands
//! are said to end, and the person's own files are read as ever.

use std::path::{Path, PathBuf};

use super::super::{DEFAULT_WAIT, Session, Until};

/// A home folder of a test's own, gone with it.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let home = std::env::temp_dir().join(format!("llx-desk-zsh-{}-{name}", std::process::id()));
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
    super::keep_in(std::env::temp_dir().join("llx-desk-test-shell"));
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
        .type_in("echo zdot=${ZDOTDIR:t} kept=${LLX_DESK_ZDOTDIR-no}\n", &[])
        .unwrap();
    let until = session.wait(None, 0, DEFAULT_WAIT);
    assert!(matches!(until, Until::Finished(_)), "{until:?}");
    let screen = session.snapshot();
    assert!(screen.contains("zdot=own kept=no"), "{screen}");
}

#[test]
fn a_shell_told_not_to_is_left_unmarked() {
    let home = Home::new("off");
    let Some(session) = zsh(&home.0, None, &[("LLX_DESK_SHELL_MARKS", "0")]) else {
        return;
    };
    session.type_in("echo done\n", &[]).unwrap();
    assert_eq!(session.wait(None, 0, DEFAULT_WAIT), Until::Settled);
}
