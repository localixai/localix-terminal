//! A shell in a tab made to say when a command starts and how it ended (`marks` reads it), with
//! nothing of the person's changed: zsh is pointed at a folder of the app's whose one file reads
//! theirs first. Another shell is started as it is, and is read if it marks its commands itself.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// What zsh reads in place of the person's `.zshenv`.
const ZSHENV: &str = include_str!("shell/zshenv");
/// Set to `0` where the shell starts, it is left as it is.
const SWITCH: &str = "LLX_DESK_SHELL_MARKS";

/// `program`, the person's shell named `shell` about to start with `vars` (this process's when
/// none), made to mark its commands if it is one the app knows how to ask.
pub(super) fn apply(program: &mut Command, shell: &str, vars: Option<&[(String, String)]>) {
    let var = |name: &str| match vars {
        Some(vars) => (vars.iter().find(|(set, _)| set == name)).map(|(_, value)| value.clone()),
        None => std::env::var(name).ok(),
    };
    if shell != "zsh" || var(SWITCH).as_deref() == Some("0") {
        return;
    }
    let Some(folder) = KEPT.get().and_then(|kept| zsh_folder(&kept.join("zsh"))) else {
        return;
    };
    // Its own ZDOTDIR is handed along, for the file to put back.
    if let Some(own) = var("ZDOTDIR") {
        program.env("LLX_DESK_ZDOTDIR", own);
    }
    program.env("ZDOTDIR", folder);
}

/// Where the app keeps what its shells read, once it is said; until then shells start unmarked.
static KEPT: OnceLock<PathBuf> = OnceLock::new();

/// The app keeps what its shells read in `folder` from now on: said once, when it starts.
pub fn keep_in(folder: PathBuf) {
    let _ = KEPT.set(folder);
}

/// `folder` with the app's `.zshenv` in it, written when it is not there as it should be; `None`
/// when it cannot be, and the shell then starts unmarked.
fn zsh_folder(folder: &Path) -> Option<PathBuf> {
    // One at a time: two tabs that start together write the one file once.
    static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _writing = crate::lock(&WRITING);
    let file = folder.join(".zshenv");
    if std::fs::read_to_string(&file).is_ok_and(|there| there == ZSHENV) {
        return Some(folder.into());
    }
    std::fs::create_dir_all(folder).ok()?;
    // Whole at once: a shell that starts meanwhile reads the old file or the new, never half.
    let draft = folder.join(format!(".zshenv.{}", std::process::id()));
    std::fs::write(&draft, ZSHENV).ok()?;
    std::fs::rename(&draft, &file).ok()?;
    Some(folder.into())
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
