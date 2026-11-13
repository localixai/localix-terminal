//! What LLX Desk, as this app was called up to 0.1.6, put in the person's tools under its own
//! name: the `llx-desk` command in `~/.local/bin`, its desktop entry on Linux, an `llx-desk`
//! server in each agent that takes MCP servers, and its Claude Code marketplace and plugin.
//! Once the update has replaced LLX Desk they run a program that is gone, and this app's own,
//! under its own name, take their place (`auto`). Each is taken out by the doctor's round, before
//! this app's are put in, when what it runs is gone or is this app; one that runs another program
//! that is there (an LLX Desk kept beside this app) is left alone. A round that leaves nothing
//! to retry is remembered in the store, and the sweep is not made again.
//!
//! What the person declined in LLX Desk needs nothing here: it is kept in the database under
//! the same ids (`declined.rs`), and the database came along (`paths::carry_over`). This can go
//! with the bridge archives of release.yml.

use std::path::{Path, PathBuf};

use super::{INTEGRATIONS, claude, command, mcp_hosts};
use crate::store::{OptionalExtension as _, Store};

/// The name LLX Desk gave its command, its servers and its desktop entry.
const FORMER: &str = "llx-desk";
/// The store's key of "the sweep was made".
const SWEPT: &str = "former.swept";

/// What one of LLX Desk's things came to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Swept {
    /// It was not there.
    Nothing,
    /// It was taken out, in words.
    Done(String),
    /// It is left as it is, and why.
    Left(String),
    /// It could not be taken out now, and why: the next round tries again.
    Failed(String),
}

/// LLX Desk's things taken out, once. What was done, in words, for the app's log.
pub(super) fn sweep() -> Vec<String> {
    if swept() {
        return vec![];
    }
    let (Ok(exe), Ok(home)) = (super::program(), super::home()) else {
        return vec![];
    };
    let mut all = vec![
        ("the command llx-desk", link(&exe, &home)),
        ("its desktop entry", desktop(&exe, &home)),
    ];
    for host in &mcp_hosts::HOSTS {
        let title = (INTEGRATIONS.iter().find(|known| known.id == host.id))
            .map_or(host.id, |known| known.title);
        all.push((title, host.retire(&home, &exe, FORMER)));
    }
    all.push(("Claude Code", claude::retire()));
    if !all
        .iter()
        .any(|(_, swept)| matches!(swept, Swept::Failed(_)))
    {
        remember();
    }
    (all.into_iter())
        .filter_map(|(what, swept)| match swept {
            Swept::Nothing => None,
            Swept::Done(done) => Some(format!("LLX Desk: {what}: {done}")),
            Swept::Left(why) => Some(format!("LLX Desk: {what} is left as it is: {why}")),
            Swept::Failed(why) => Some(format!("LLX Desk: {what} was not taken out: {why}")),
        })
        .collect()
}

/// `~/.local/bin/llx-desk` taken away when it is a link of LLX Desk's that runs nothing now (its
/// program, `llx-desk`, gone), or one that runs this app: the same as `command::link` would
/// replace. A file that is no link is never removed.
pub(super) fn link(exe: &Path, home: &Path) -> Swept {
    let at = home.join(".local").join("bin").join(FORMER);
    let Ok(meta) = std::fs::symlink_metadata(&at) else {
        return Swept::Nothing;
    };
    let shown = at.display();
    if !meta.file_type().is_symlink() {
        return Swept::Left(format!("{shown} is no link"));
    }
    let to = std::fs::read_link(&at).unwrap_or_default();
    let ours = match std::fs::canonicalize(&at) {
        Err(_) => to.file_name().is_some_and(|name| name == FORMER),
        Ok(found) => found == exe || command::is_llxterm(&at),
    };
    if !ours {
        return Swept::Left(format!("{shown} runs {}", to.display()));
    }
    match std::fs::remove_file(&at) {
        Ok(()) => Swept::Done(format!("{shown} is gone")),
        Err(why) => Swept::Failed(format!("{shown}: {why}")),
    }
}

/// LLX Desk's desktop entry (`~/.local/share/applications/llx-desk.desktop`, on Linux) taken
/// away when it is LLX Desk's and the program it opens is gone or is this app.
pub(super) fn desktop(exe: &Path, home: &Path) -> Swept {
    let file = home.join(".local/share/applications/llx-desk.desktop");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Swept::Nothing;
    };
    let shown = file.display();
    let program = (text.lines())
        .find_map(|line| line.strip_prefix("Exec=\""))
        .and_then(|rest| rest.split_once('"'))
        .map(|(program, _)| PathBuf::from(program));
    let ours = text.lines().any(|line| line == "Name=LLX Desk")
        && program.is_some_and(|program| mcp_hosts::gone_or_this(&program.to_string_lossy(), exe));
    if !ours {
        return Swept::Left(format!("{shown} opens a program that is there"));
    }
    match std::fs::remove_file(&file) {
        Ok(()) => Swept::Done(format!("{shown} is gone")),
        Err(why) => Swept::Failed(format!("{shown}: {why}")),
    }
}

/// Whether the sweep was made. A store that cannot be read counts as made: nothing is taken out
/// on a guess.
fn swept() -> bool {
    let read = Store::open(&crate::paths::database()).and_then(|store| {
        store.read(|db| {
            db.query_row("SELECT 1 FROM state WHERE key = ?1", [SWEPT], |_| Ok(()))
                .optional()
                .map(|found| found.is_some())
                .map_err(|why| why.to_string())
        })
    });
    read.unwrap_or_else(|why| {
        log::warn!("setup: whether LLX Desk's leftovers were taken out is unknown: {why}");
        true
    })
}

/// The sweep remembered as made.
fn remember() {
    let written = Store::open(&crate::paths::database()).and_then(|store| {
        store.write(|tx| {
            tx.execute(
                "INSERT INTO state (key, value) VALUES (?1, '1')
                 ON CONFLICT (key) DO UPDATE SET value = '1'",
                [SWEPT],
            )
            .map(drop)
            .map_err(|why| why.to_string())
        })
    });
    if let Err(why) = written {
        log::warn!("setup: that LLX Desk's leftovers were taken out is not remembered: {why}");
    }
}

#[cfg(test)]
#[path = "former_tests.rs"]
mod tests;
