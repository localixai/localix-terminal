//! A skill put where an agent reads the person's skills: its folder copied as `<skills>/<name>`,
//! whole or not at all (copied beside it, then renamed into place), with a mark of this app's
//! inside (`.llxterm.json`: the listing and its version). The mark is how a folder is known to
//! be this app's: one without it is the person's own, and is never written over or removed; one
//! of another listing of the same name is left too. The name is one safe path segment, as the
//! skills' own format has it, and the copy never follows a link out of the folder it is given.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::Agent;
use crate::catalog::capital;
use crate::catalog::{Env, Listing, Plan, With};
use crate::setup::reach::shown;

/// The file that marks a skill's folder as this app's.
const MARK: &str = ".llxterm.json";

/// What is at a skill's place.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum There {
    Nothing,
    /// This listing, put there by this app: its version.
    Ours(String),
    /// Another listing put there by this app: its id.
    Another(String),
    /// A folder, a file or a link that is not this app's.
    Persons,
}

/// Whether `name` is a skill's name as its format has it: 1 to 64 of `a-z`, `0-9` and `-`,
/// with no `-` first, last or twice in a row; so it is one segment of a path, never `..`.
pub(super) fn safe(name: &str) -> Result<(), String> {
    let fits = (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--");
    match fits {
        true => Ok(()),
        false => Err(format!("`{name}` is no skill's name (a-z, 0-9 and -)")),
    }
}

/// The skill's place for `agent`.
fn place(agent: &Agent, listing: &Listing, env: &Env) -> Result<PathBuf, String> {
    safe(&listing.name)?;
    Ok((agent.skills)(env).join(&listing.name))
}

/// What is at `folder`, for `listing`. A link is never followed: it is the person's.
pub(super) fn there(folder: &Path, listing: &Listing) -> There {
    let Ok(meta) = std::fs::symlink_metadata(folder) else {
        return There::Nothing;
    };
    if !meta.is_dir() {
        return There::Persons;
    }
    let mark = std::fs::read_to_string(folder.join(MARK)).ok();
    let mark: Option<Value> = mark.and_then(|text| serde_json::from_str(&text).ok());
    let Some(listed) = (mark.as_ref()).and_then(|mark| mark["listing"].as_str()) else {
        return There::Persons;
    };
    match listed == listing.id {
        true => There::Ours(
            (mark.as_ref())
                .and_then(|mark| mark["version"].as_str())
                .unwrap_or_default()
                .to_string(),
        ),
        false => There::Another(listed.to_string()),
    }
}

/// Why `folder` is left as it is, when it is not this listing's.
fn refused(folder: &Path, there: &There, env: &Env) -> Option<String> {
    let at = shown(folder, &env.home);
    match there {
        There::Nothing | There::Ours(_) => None,
        There::Another(id) => Some(format!(
            "{at} holds the skill of another listing ({id}): remove that one first"
        )),
        There::Persons => Some(format!(
            "{at} is a skill Localix Terminal did not install: it is left as it is"
        )),
    }
}

/// `step`, with who else reads the folder when it is shared.
fn with_shared(agent: &Agent, step: String) -> String {
    match agent.shared {
        "" => step,
        shared => format!("{step}, which {shared} both read"),
    }
}

pub(super) fn plan(agent: &Agent, listing: &Listing, env: &Env) -> Result<Plan, String> {
    let folder = place(agent, listing, env)?;
    let there = there(&folder, listing);
    if let Some(why) = refused(&folder, &there, env) {
        return Err(why);
    }
    let at = shown(&folder, &env.home);
    let step = match there {
        There::Ours(version) if !version.is_empty() => {
            format!("Replace the skill in {at} (version {version})")
        }
        There::Ours(_) => format!("Replace the skill in {at}"),
        There::Nothing | There::Another(_) | There::Persons => format!("Copy the skill to {at}"),
    };
    Ok(Plan {
        steps: vec![with_shared(agent, step)],
        after: capital(agent.after),
    })
}

pub(super) fn install(
    agent: &Agent,
    listing: &Listing,
    with: &With,
    env: &Env,
) -> Result<String, String> {
    let folder = place(agent, listing, env)?;
    let from = with
        .folder
        .as_deref()
        .ok_or("the skill's folder was not fetched")?;
    let source =
        std::fs::symlink_metadata(from).map_err(|why| format!("{}: {why}", from.display()))?;
    let holds = std::fs::symlink_metadata(from.join("SKILL.md")).is_ok_and(|meta| meta.is_file());
    if !source.is_dir() || !holds {
        return Err(format!("{} holds no SKILL.md", from.display()));
    }
    let there = there(&folder, listing);
    if let Some(why) = refused(&folder, &there, env) {
        return Err(why);
    }
    let parent = folder.parent().ok_or("a skill's place has no folder")?;
    std::fs::create_dir_all(parent).map_err(|why| format!("{}: {why}", parent.display()))?;
    let made = beside(&folder, "new");
    let copied = copy_tree(from, &made).and_then(|()| {
        let mark = json!({"listing": listing.id, "version": listing.version});
        std::fs::write(made.join(MARK), format!("{mark}\n"))
    });
    if let Err(why) = copied {
        let _ = std::fs::remove_dir_all(&made);
        return Err(format!("the skill could not be copied: {why}"));
    }
    put(&made, &folder, matches!(there, There::Ours(_)))
        .map_err(|why| format!("{}: {why}", folder.display()))?;
    let at = shown(&folder, &env.home);
    Ok(format!(
        "{}: {}.",
        with_shared(agent, format!("Copied the skill to {at}")),
        agent.after
    ))
}

/// `made` put at `folder`, the one there before (this app's) taken away once it is; it is put
/// back when the new one cannot be.
fn put(made: &Path, folder: &Path, replacing: bool) -> std::io::Result<()> {
    if !replacing {
        return std::fs::rename(made, folder).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(made);
        });
    }
    let old = beside(folder, "old");
    std::fs::rename(folder, &old)?;
    if let Err(why) = std::fs::rename(made, folder) {
        let _ = std::fs::rename(&old, folder);
        let _ = std::fs::remove_dir_all(made);
        return Err(why);
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

pub(super) fn remove(agent: &Agent, listing: &Listing, env: &Env) -> Result<String, String> {
    let folder = place(agent, listing, env)?;
    let there = there(&folder, listing);
    if let Some(why) = refused(&folder, &there, env) {
        return Err(why);
    }
    let at = shown(&folder, &env.home);
    if there == There::Nothing {
        return Ok(format!("{} has no skill at {at}.", agent.name));
    }
    // Moved aside first, so the agent never reads half a folder.
    let gone = beside(&folder, "gone");
    std::fs::rename(&folder, &gone).map_err(|why| format!("{at}: {why}"))?;
    std::fs::remove_dir_all(&gone).map_err(|why| format!("{at}: {why}"))?;
    Ok(format!(
        "{}.",
        with_shared(agent, format!("Removed the skill from {at}"))
    ))
}

pub(super) fn has(agent: &Agent, listing: &Listing, env: &Env) -> Option<String> {
    match there(&place(agent, listing, env).ok()?, listing) {
        There::Ours(version) => Some(version),
        There::Nothing | There::Another(_) | There::Persons => None,
    }
}

/// A hidden name beside `folder`, of this run's own, for a folder on its way in or out.
fn beside(folder: &Path, what: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let name = folder.file_name().unwrap_or_default().to_string_lossy();
    let nanos = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
        .map_or(0, |since| since.subsec_nanos());
    let tag = format!(
        "{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    folder.with_file_name(format!(".{name}.llx-{what}-{tag}"))
}

/// The folder `from` copied to `to`, which is made new: its folders and files, never a link
/// (one is left out, so nothing outside `from` is read).
#[expect(
    clippy::create_dir,
    reason = "the copy is made new: a folder already there is not written into"
)]
#[expect(
    clippy::filetype_is_file,
    reason = "only plain files are copied: a link, a pipe or a socket is left out"
)]
pub(super) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
