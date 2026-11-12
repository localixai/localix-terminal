//! The `llxterm` command on the person's PATH: a link in `~/.local/bin` to this program (in the
//! app, the program of the bundle), that directory added to their login shell's profile when
//! the shell does not have it. No password: it is all the person's own.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::State;
use super::shell::{login_path, on_path, within};

/// Where the link goes.
pub(super) fn link_path(home: &Path) -> PathBuf {
    home.join(".local").join("bin").join("llxterm")
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

/// `exe` as `llxterm` on the person's PATH; what was done, in words.
pub(super) fn put_on_path(exe: &Path, home: &Path) -> Result<String, String> {
    let at = link_path(home);
    let linked = link(exe, &at)?;
    let bin = at.parent().unwrap_or(home);
    match login_path() {
        Some(path) if on_path(&path, bin) => return Ok(linked),
        Some(_) => {}
        // A profile is not edited for a PATH that could not be read: it may have the directory.
        None => {
            return Ok(format!(
                "{linked}; could not read the PATH of your login shell: profile left alone"
            ));
        }
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

/// The link taken away, when it leads to this program (as `state` finds it: through any chain
/// of links). A file that is no link is never removed.
pub(super) fn take_off_path(exe: &Path, home: &Path) -> Result<String, String> {
    let at = link_path(home);
    let is_link = std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_symlink());
    if is_link && std::fs::canonicalize(&at).is_ok_and(|to| to == exe) {
        std::fs::remove_file(&at).map_err(|why| format!("{}: {why}", at.display()))?;
        return Ok(format!("{} is gone", at.display()));
    }
    Ok(format!("{} was not this app's", at.display()))
}

/// `link` made a link to `exe`: made, kept when it is one already, or put in place of another
/// llxterm (an older app's link, a dangling one, or a copy of the program). Anything else there
/// is left alone.
pub(super) fn link(exe: &Path, link: &Path) -> Result<String, String> {
    let shown = link.display();
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    }
    match std::fs::symlink_metadata(link) {
        Ok(meta)
            if meta.file_type().is_symlink()
                && std::fs::read_link(link).is_ok_and(|to| to == exe) =>
        {
            return Ok(format!("{shown} is linked already"));
        }
        // A link that leads nowhere is replaced; one that leads to a program is, when it is an
        // llxterm.
        Ok(_) if link.exists() && !is_llxterm(link) => {
            return Err(format!("{shown} is another program: left as it is"));
        }
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("{shown}: {why}")),
    }
    // A new link moved over the old: there is always an `llxterm` there.
    let new = link.with_extension(format!("new{}", std::process::id()));
    let _ = std::fs::remove_file(&new);
    std::os::unix::fs::symlink(exe, &new).map_err(|why| format!("{}: {why}", new.display()))?;
    std::fs::rename(&new, link).map_err(|why| format!("{shown}: {why}"))?;
    Ok(format!("{shown} links to {}", exe.display()))
}

/// Where the desktop entry goes (Linux).
#[cfg(target_os = "linux")]
fn desktop_file(home: &Path) -> PathBuf {
    home.join(".local/share/applications/llxterm.desktop")
}

/// The line of the desktop entry that runs `exe`.
#[cfg(target_os = "linux")]
fn desktop_exec(exe: &Path) -> String {
    format!("Exec=\"{}\" open", exe.display())
}

/// The app among the person's applications (Linux): a desktop entry that opens the window.
#[cfg(target_os = "linux")]
pub(super) fn desktop_entry(exe: &Path, home: &Path) -> Result<String, String> {
    let file = desktop_file(home);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Localix Terminal\nComment=Pages and terminals agents work in, and you watch\n{}\nTerminal=false\nCategories=Development;\n",
        desktop_exec(exe)
    );
    std::fs::write(&file, entry).map_err(|why| format!("{}: {why}", file.display()))?;
    Ok(format!("{} opens the app", file.display()))
}

/// The desktop entry taken away, when it runs this program (Linux).
#[cfg(target_os = "linux")]
pub(super) fn take_off_entry(exe: &Path, home: &Path) -> Result<String, String> {
    let file = desktop_file(home);
    let ours = std::fs::read_to_string(&file).is_ok_and(|text| text.contains(&desktop_exec(exe)));
    if !ours {
        return Ok(format!("{} was not this app's", file.display()));
    }
    std::fs::remove_file(&file).map_err(|why| format!("{}: {why}", file.display()))?;
    Ok(format!("{} is gone", file.display()))
}

/// Whether the program at `file` is an llxterm, by what its `--version` says.
fn is_llxterm(file: &Path) -> bool {
    let mut asked = std::process::Command::new(file);
    asked.arg("--version");
    within(asked, Duration::from_secs(5)).is_ok_and(|ran| ran.out.starts_with("llxterm "))
}

/// The profile a login shell of `shell` reads: zsh's `.zprofile`; bash's first of `.bash_profile`,
/// `.bash_login` and `.profile` that is there (a new `.bash_profile` would hide `.profile`, which
/// bash then no longer reads), `.bash_profile` when none is; else `.profile`.
pub(super) fn profile_of(shell: &str, home: &Path) -> PathBuf {
    let name = match shell.rsplit('/').next() {
        Some("zsh") => ".zprofile",
        Some("bash") => {
            let there = [".bash_profile", ".bash_login", ".profile"]
                .into_iter()
                .find(|name| home.join(name).exists());
            there.unwrap_or(".bash_profile")
        }
        _ => ".profile",
    };
    home.join(name)
}

/// The line that puts ~/.local/bin on the PATH, at the end of `profile`, once; whether it was
/// added now.
pub(super) fn add_to_profile(profile: &Path) -> Result<bool, String> {
    const MARK: &str = "# Added by Localix Terminal: its command, llxterm, in ~/.local/bin.";
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
