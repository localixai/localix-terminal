//! Projects: the scope of everything the window holds (docs/projects.md). A project is a folder,
//! known by its canonical path and kept as a row: its tabs, the pages they showed, its board and
//! its events are its own. Home, the project of the home folder, is where what has no project goes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::store::{Connection, OptionalExtension, Transaction, params};

/// A project's row id.
pub type ProjectId = i64;

/// A project as it is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub path: PathBuf,
    /// Its folder's name.
    pub name: String,
}

/// The project folder `dir` is in: the nearest folder at or above it with a `.git`, else `dir`.
pub fn root_of(dir: &Path) -> PathBuf {
    let found = dir.ancestors().find(|at| at.join(".git").exists());
    found.unwrap_or(dir).to_path_buf()
}

/// The home folder: Home's.
pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// `path` as a project is known by it: absolute, its links resolved.
///
/// # Errors
/// It is not absolute, or there is no such folder.
pub fn canonical(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "a project is an absolute path, not `{}`",
            path.display()
        ));
    }
    let found = std::fs::canonicalize(path)
        .map_err(|why| format!("no project at {}: {why}", path.display()))?;
    match found.is_dir() {
        true => Ok(found),
        false => Err(format!("{} is not a folder", found.display())),
    }
}

/// The name of the folder at `path`.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let path: String = row.get(1)?;
    Ok(Project {
        id: row.get(0)?,
        path: PathBuf::from(path),
        name: row.get(2)?,
    })
}

/// The project at canonical `path`, made the first time; opened `now`.
///
/// # Errors
/// SQLite could not.
pub fn find_or_make(tx: &Transaction<'_>, path: &Path, now: u64) -> Result<Project, String> {
    let text = |why: rusqlite::Error| why.to_string();
    let shown = path.display().to_string();
    tx.execute(
        "INSERT INTO projects (path, name, created, opened) VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT (path) DO UPDATE SET opened = excluded.opened",
        params![shown, name_of(path), now],
    )
    .map_err(text)?;
    tx.query_row(
        "SELECT id, path, name FROM projects WHERE path = ?1",
        [shown],
        row,
    )
    .map_err(text)
}

/// Project `id` opened `now`: the person went to it, the first of the recent ones.
///
/// # Errors
/// SQLite could not.
pub fn opened(tx: &Transaction<'_>, id: ProjectId, now: u64) -> Result<(), String> {
    tx.execute(
        "UPDATE projects SET opened = ?2 WHERE id = ?1",
        params![id, now],
    )
    .map_err(|why| why.to_string())?;
    Ok(())
}

/// Project `id`, while it is kept.
///
/// # Errors
/// SQLite could not.
pub fn get(db: &Connection, id: ProjectId) -> Result<Option<Project>, String> {
    db.query_row(
        "SELECT id, path, name FROM projects WHERE id = ?1",
        [id],
        row,
    )
    .optional()
    .map_err(|why| why.to_string())
}

/// The projects, the one opened last first, at most `most`.
///
/// # Errors
/// SQLite could not.
pub fn recent(db: &Connection, most: usize) -> Result<Vec<Project>, String> {
    let text = |why: rusqlite::Error| why.to_string();
    let mut rows = db
        .prepare_cached(
            "SELECT id, path, name FROM projects ORDER BY opened DESC, id DESC LIMIT ?1",
        )
        .map_err(text)?;
    let most = i64::try_from(most).unwrap_or(i64::MAX);
    let read = rows.query_map([most], row).map_err(text)?;
    read.collect::<Result<_, _>>().map_err(text)
}

/// Project `id` forgotten, with everything that is its own (its tabs, pages, board, events).
///
/// # Errors
/// SQLite could not.
pub fn forget(tx: &Transaction<'_>, id: ProjectId) -> Result<(), String> {
    tx.execute("DELETE FROM projects WHERE id = ?1", [id])
        .map(drop)
        .map_err(|why| why.to_string())
}

/// Who calls, as the desk keeps their tabs and notes apart: an agent of a project. The project's
/// own agent is known by the project's id; a named one (a worker) by the id and its name. An id
/// never holds a `/`, so no folder's name or agent's makes one owner look like another.
pub fn owner(project: &Project, agent: Option<&str>) -> String {
    match agent.filter(|name| !name.is_empty()) {
        Some(name) => format!("{}/{name}", project.id),
        None => project.id.to_string(),
    }
}

/// Project `id`'s key in the window, which groups its tabs by it.
pub fn key(id: ProjectId) -> String {
    id.to_string()
}

/// The project a window key names.
pub fn of_key(key: &str) -> Option<ProjectId> {
    key.parse().ok()
}

/// What each project is called in the window: its folder's name, and where two share one, the
/// folder above too (`app — work`).
pub fn labels(projects: &[Project]) -> HashMap<ProjectId, String> {
    let mut named: HashMap<&str, usize> = HashMap::new();
    for project in projects {
        *named.entry(project.name.as_str()).or_default() += 1;
    }
    (projects.iter())
        .map(|project| {
            let parent = (project.path.parent())
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned());
            let label = match (named.get(project.name.as_str()), parent) {
                (Some(count), Some(parent)) if *count > 1 => {
                    format!("{} — {parent}", project.name)
                }
                _ => project.name.clone(),
            };
            (project.id, label)
        })
        .collect()
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
