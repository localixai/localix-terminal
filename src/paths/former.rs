//! What LLX Desk, as this app was called up to 0.1.6, kept under its own names, carried over to
//! this app's once, before anything opens them: its data folder (`~/.local/share/llx-desk`)
//! and the database in it (`llx-desk.sqlite`, with its `-wal` and `-shm`), its settings
//! (`~/.config/llx-desk`), and on Linux the program 0.1.6's updater left in its folder. Each
//! move is a rename, all of it or none: what cannot be moved stays where it was under its old
//! name, and the lines returned say what was done, for the app's log.
//!
//! A folder the person named (`LLXTERM_DATA`, or 0.1.6's `LLX_DESK_DATA`) stays where it is;
//! only the database in it takes its new name. This can go, with the old variables in
//! `paths.rs`, once no one updates from 0.1.6 any more: when release.yml stops publishing the
//! bridge archives.

use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::io::AsRawFd as _;
use std::path::{Path, PathBuf};

/// The name 0.1.6 gave its folders, its runtime directory and its program.
pub(super) const FORMER: &str = "llx-desk";
/// This app's name for the same.
const NAME: &str = "llxterm";
/// The database's files, the old name and the new: moved together, or none of them (SQLite
/// finds its journal by the database's name).
const DATABASE: [(&str, &str); 3] = [
    ("llx-desk.sqlite", "llxterm.sqlite"),
    ("llx-desk.sqlite-wal", "llxterm.sqlite-wal"),
    ("llx-desk.sqlite-shm", "llxterm.sqlite-shm"),
];
/// The program 0.1.6's install.sh and updater kept in the data folder on Linux, and what its
/// updater left beside it: no data, so never carried into a folder this app made itself.
const PROGRAM: [&str; 3] = ["llx-desk", "llx-desk.old", "llx-desk.new"];

/// Everything of 0.1.6's carried over, as the process's variables place it. A test build and a
/// build for development carry nothing: what they would move is the person's, in use by the app
/// they run. What was done, in words.
pub fn carry_over() -> Vec<String> {
    if cfg!(feature = "test-server") || cfg!(debug_assertions) {
        return vec![];
    }
    let mut said = vec![];
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let (None, Some(home)) = (super::moved_data(), &home) {
        let share = home.join(".local").join("share");
        said.extend(folder(&share.join(FORMER), &share.join(NAME), true));
    }
    said.extend(database(&super::data()));
    if let Some(config) = super::config() {
        said.extend(folder(&config.with_file_name(FORMER), &config, false));
    }
    let share = (std::env::var_os("XDG_DATA_HOME").filter(|dir| !dir.is_empty()))
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".local").join("share")));
    if let (Some(share), Ok(exe)) = (share, std::env::current_exe()) {
        said.extend(program(&share, &exe));
    }
    said
}

/// The process that serves as LLX Desk 0.1.6, when one holds its lock (`llx-desk-<uid>` in the
/// user's temporary directory): its data must not move under it.
pub fn former_server() -> Option<libc::pid_t> {
    if cfg!(feature = "test-server") {
        return None;
    }
    // SAFETY: getuid takes nothing and cannot fail.
    let uid = unsafe { libc::getuid() };
    let dir = super::user_temp().join(format!("{FORMER}-{uid}"));
    held_by(&dir.join("desk.lock"))
}

/// The pid written in `lock` while a process holds it; none when it is free or not there.
pub(super) fn held_by(lock: &Path) -> Option<libc::pid_t> {
    let file = std::fs::File::open(lock).ok()?;
    // A shared hold is refused while another has the lock; taken, it goes with `file`.
    // SAFETY: flock on a descriptor this function owns.
    let free = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } == 0;
    if free {
        return None;
    }
    let written = std::fs::read_to_string(lock).ok()?;
    written.trim().parse().ok().filter(|pid| *pid > 0)
}

/// Whether anything is at `path`, a link that leads nowhere included.
fn there(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// The folder `old` carried to `new`: renamed whole when `new` is not there; else what `new`
/// lacks moved into it, unless it holds a database of its own (`data`), when the person has
/// used this app already and `old` is left as it is.
pub(super) fn folder(old: &Path, new: &Path, data: bool) -> Vec<String> {
    if !old.is_dir() {
        return vec![];
    }
    let (from, to) = (old.display(), new.display());
    match std::fs::symlink_metadata(new) {
        Err(why) if why.kind() == io::ErrorKind::NotFound => whole(old, new, data),
        Err(why) => vec![format!("{from} stays: {to} cannot be read ({why})")],
        Ok(_) if !new.is_dir() => vec![format!("{from} stays: {to} is no folder")],
        Ok(_) if data && there(&new.join(DATABASE[0].1)) => {
            vec![format!(
                "{from} stays as it was: {to} has a database of its own"
            )]
        }
        Ok(_) => merged(old, new),
    }
}

/// `old` renamed `new`, then its database renamed in it; back as it was if that fails.
fn whole(old: &Path, new: &Path, data: bool) -> Vec<String> {
    let (from, to) = (old.display(), new.display());
    if let Err(why) = std::fs::rename(old, new) {
        return vec![format!(
            "{from} stays: it could not be renamed {to} ({why})"
        )];
    }
    let renamed = match data {
        true => database_between(new, new),
        false => Ok(false),
    };
    match renamed {
        Ok(_) => vec![format!("{from} is now {to}")],
        Err(why) => {
            let back = std::fs::rename(new, old);
            let then = back.map_or_else(
                |undone| format!(", and it stays at {to}: {undone}"),
                |()| String::new(),
            );
            vec![format!("{from} stays: {why}{then}")]
        }
    }
}

/// What of `old` that `new` lacks moved into it, the database first, under their new names;
/// `old` removed when nothing is left in it.
fn merged(old: &Path, new: &Path) -> Vec<String> {
    let (from, to) = (old.display(), new.display());
    let mut said = vec![];
    match database_between(old, new) {
        Ok(true) => said.push(format!("the database of {from} is now in {to}")),
        Ok(false) => {}
        Err(why) => said.push(format!("the database stays in {from}: {why}")),
    }
    let entries = std::fs::read_dir(old).map_or_else(|_| vec![], |dir| dir.flatten().collect());
    for entry in entries {
        let name = entry.file_name();
        let database = DATABASE.iter().map(|(old, _)| old);
        if PROGRAM.iter().chain(database).any(|known| name == *known) {
            continue;
        }
        let into = new.join(&name);
        let shown = name.to_string_lossy();
        if there(&into) {
            said.push(format!("{from}/{shown} stays: {to} has its own"));
        } else if let Err(why) = std::fs::rename(entry.path(), &into) {
            said.push(format!("{from}/{shown} stays: {why}"));
        } else {
            said.push(format!("{from}/{shown} is now in {to}"));
        }
    }
    // Only an empty folder is removed.
    let _ = std::fs::remove_dir(old);
    said
}

/// The database in `dir` under its new name, when it is there under its old one and not yet
/// under the new. What was done, in words.
pub(super) fn database(dir: &Path) -> Vec<String> {
    match database_between(dir, dir) {
        Ok(true) => vec![format!(
            "the database in {} is now {}",
            dir.display(),
            DATABASE[0].1
        )],
        Ok(false) => vec![],
        Err(why) => vec![format!("the database in {} stays: {why}", dir.display())],
    }
}

/// The database's files moved from `from` under their old names to `to` under their new ones:
/// all of them, or none (those moved are moved back). Whether there was one to move; one there
/// under the new name already is left alone, and so is the old.
fn database_between(from: &Path, to: &Path) -> Result<bool, String> {
    if !there(&from.join(DATABASE[0].0)) || there(&to.join(DATABASE[0].1)) {
        return Ok(false);
    }
    if let Some((_, new)) = DATABASE.iter().find(|(_, new)| there(&to.join(new))) {
        return Err(format!("{} is there already", to.join(new).display()));
    }
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    for (old, new) in DATABASE {
        let (old, new) = (from.join(old), to.join(new));
        if !there(&old) {
            continue;
        }
        if let Err(why) = std::fs::rename(&old, &new) {
            for (old, new) in moved.iter().rev() {
                let _ = std::fs::rename(new, old);
            }
            return Err(format!("{} could not be renamed: {why}", old.display()));
        }
        moved.push((old, new));
    }
    Ok(true)
}

/// On Linux, the program 0.1.6's updater put at `share/llx-desk/llx-desk` (carried with the data
/// folder to `share/llxterm/llx-desk`, where the two are one) renamed where install.sh puts this
/// app's, `share/llxterm/llxterm`, so that this app finds itself installed and updates. Only
/// the program that runs (`exe`) is moved, and only to a place nothing holds.
pub(super) fn program(share: &Path, exe: &Path) -> Vec<String> {
    let to = share.join(NAME).join(NAME);
    if there(&to) {
        return vec![];
    }
    let places = [
        share.join(NAME).join(FORMER),
        share.join(FORMER).join(FORMER),
    ];
    let Some(from) = places.into_iter().find(|from| same_file(from, exe)) else {
        return vec![];
    };
    let moved = (to.parent().map_or(Ok(()), std::fs::create_dir_all))
        .and_then(|()| std::fs::rename(&from, &to));
    let (from_shown, to_shown) = (from.display(), to.display());
    match moved {
        Ok(()) => {
            let _ = std::fs::remove_dir(share.join(FORMER));
            vec![format!("the program {from_shown} is now {to_shown}")]
        }
        Err(why) => vec![format!("the program stays at {from_shown}: {why}")],
    }
}

/// Whether `a` and `b` are one file.
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "former_tests.rs"]
mod tests;
