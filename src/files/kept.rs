//! The folders a person left open in each project's tree, kept by their path under the
//! project's folder. A folder is open only under open folders: shutting one shuts what is
//! under it, and one renamed or moved stays open where it is now, with what is open under it.

use crate::projects::ProjectId;
use crate::store::{Connection, Transaction, params};

/// Project `project`'s open folders, each a path under its folder, a parent before its own.
pub fn open_dirs(db: &Connection, project: ProjectId) -> Result<Vec<String>, String> {
    let mut read = db
        .prepare_cached("SELECT path FROM open_dirs WHERE project = ?1 ORDER BY path")
        .map_err(|why| why.to_string())?;
    let rows = read
        .query_map(params![project], |row| row.get::<_, String>(0))
        .map_err(|why| why.to_string())?;
    rows.collect::<Result<_, _>>()
        .map_err(|why| why.to_string())
}

/// Folder `path` of `project` kept open; whether it was not already.
pub fn open(tx: &Transaction<'_>, project: ProjectId, path: &str) -> Result<bool, String> {
    tx.execute(
        "INSERT OR IGNORE INTO open_dirs (project, path) VALUES (?1, ?2)",
        params![project, path],
    )
    .map(|rows| rows > 0)
    .map_err(|why| why.to_string())
}

/// Folder `path` of `project` shut, and every folder under it; whether any was open.
pub fn shut(tx: &Transaction<'_>, project: ProjectId, path: &str) -> Result<bool, String> {
    tx.execute(
        "DELETE FROM open_dirs WHERE project = ?1
         AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
        params![project, path],
    )
    .map(|rows| rows > 0)
    .map_err(|why| why.to_string())
}

/// Folder `from` of `project` is at `to` now: it and every folder under it stay as open as
/// they were, by their new paths. What was kept of `to` before is replaced.
pub fn moved(tx: &Transaction<'_>, project: ProjectId, from: &str, to: &str) -> Result<(), String> {
    tx.execute(
        "DELETE FROM open_dirs WHERE project = ?1
         AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
        params![project, to],
    )
    .map_err(|why| why.to_string())?;
    tx.execute(
        "UPDATE open_dirs SET path = ?3 || substr(path, length(?2) + 1) WHERE project = ?1
         AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
        params![project, from, to],
    )
    .map(drop)
    .map_err(|why| why.to_string())
}
