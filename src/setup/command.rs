//! The `llx-desk` command on the person's PATH: a link in `~/.local/bin` to this program (in the
//! app, the program of the bundle), that directory added to their login shell's profile when
//! the shell does not have it. No password: it is all the person's own.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::State;
use super::shell::{login_path, on_path, within};

/// Where the link goes.
pub(super) fn link_path(home: &Path) -> PathBuf {
    home.join(".local").join("bin").join("llx-desk")
}

/// Whether the command is this program, and on the PATH of a new terminal.
pub(super) fn state(exe: &Path, home: &Path) -> State {
    let at = link_path(home);
    let linked = std::fs::canonicalize(&at).is_ok_and(|to| to == exe);
    let bin = at.parent().unwrap_or(home);
    match (linked, login_path().is_some_and(|path| on_path(&path, bin))) {
        (true, true) => State::On(format!("{} runs this app's program", at.display())),
        (true, false) => {
            State::Off("linked, but a new terminal's PATH does not have ~/.local/bin".into())
        }
        (false, _) => State::Off(format!("{} is not this app's program", at.display())),
    }
}

/// `exe` as `llx-desk` on the person's PATH; what was done, in words.
pub(super) fn put_on_path(exe: &Path, home: &Path) -> Result<String, String> {
    let at = link_path(home);
    let linked = link(exe, &at)?;
    let bin = at.parent().unwrap_or(home);
    if login_path().is_some_and(|path| on_path(&path, bin)) {
        return Ok(linked);
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let profile = profile_of(&shell, home);
    match add_to_profile(&profile)? {
        true => Ok(format!(
            "{linked}; {} puts ~/.local/bin on the PATH",
            profile.display()
        )),
        false => Ok(linked),
    }
}

/// The link taken away, when it is this program's.
pub(super) fn take_off_path(exe: &Path, home: &Path) -> Result<String, String> {
    let at = link_path(home);
    if std::fs::read_link(&at).is_ok_and(|to| to == exe) {
        std::fs::remove_file(&at).map_err(|why| format!("{}: {why}", at.display()))?;
        return Ok(format!("{} is gone", at.display()));
    }
    Ok(format!("{} was not this app's", at.display()))
}

/// `link` made a link to `exe`: made, kept when it is one already, or put in place of another
/// llx-desk (an older app's link, or a copy of the program). Anything else there is left alone.
pub(super) fn link(exe: &Path, link: &Path) -> Result<String, String> {
    let shown = link.display();
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    }
    match std::fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if std::fs::read_link(link).is_ok_and(|to| to == exe) {
                return Ok(format!("{shown} is linked already"));
            }
        }
        Ok(_) if !is_llx_desk(link) => {
            return Err(format!("{shown} is another program: left as it is"));
        }
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("{shown}: {why}")),
    }
    // A new link moved over the old: there is always an `llx-desk` there.
    let new = link.with_extension("new");
    let _ = std::fs::remove_file(&new);
    std::os::unix::fs::symlink(exe, &new).map_err(|why| format!("{}: {why}", new.display()))?;
    std::fs::rename(&new, link).map_err(|why| format!("{shown}: {why}"))?;
    Ok(format!("{shown} links to {}", exe.display()))
}

/// Whether the program at `file` is an llx-desk, by what its `--version` says.
fn is_llx_desk(file: &Path) -> bool {
    let mut asked = std::process::Command::new(file);
    asked.arg("--version");
    within(asked, Duration::from_secs(5)).is_some_and(|ran| ran.out.starts_with("llx-desk "))
}

/// The profile a login shell of `shell` reads: zsh's `.zprofile`, bash's `.bash_profile`, else
/// `.profile`.
pub(super) fn profile_of(shell: &str, home: &Path) -> PathBuf {
    let name = match shell.rsplit('/').next() {
        Some("zsh") => ".zprofile",
        Some("bash") => ".bash_profile",
        _ => ".profile",
    };
    home.join(name)
}

/// The line that puts ~/.local/bin on the PATH, at the end of `profile`, once; whether it was
/// added now.
pub(super) fn add_to_profile(profile: &Path) -> Result<bool, String> {
    const MARK: &str = "# Added by LLX Desk: its command, llx-desk, in ~/.local/bin.";
    let had = std::fs::read_to_string(profile).unwrap_or_default();
    if had.contains(MARK) {
        return Ok(false);
    }
    let gap = if had.is_empty() || had.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let text = format!("{gap}\n{MARK}\nexport PATH=\"$HOME/.local/bin:$PATH\"\n");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile)
        .map_err(|why| format!("{}: {why}", profile.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|why| format!("{}: {why}", profile.display()))?;
    Ok(true)
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
