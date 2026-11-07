//! Where a board is kept: in the app's database, one row a project, written in the same
//! transaction as the notice that it changed.
//!
//! A project's board is also written out to `.localix/board.json` in its folder, for an
//! integration to read (`llx-desk window --board`) and for git to keep: an export, made from the
//! row after each change and made again from the rows when the desk starts. The desk reads it back
//! only once: a project whose board has no row yet takes its file as the board it had before.

use std::path::{Path, PathBuf};

use super::Board;
use crate::projects::ProjectId;
use crate::store::{Connection, OptionalExtension, Transaction, params};

/// Where the board of the project at `project` is written out.
pub fn place(project: &Path) -> PathBuf {
    project.join(".localix").join("board.json")
}

/// Project `project`'s board as its row holds it; none where it has no row.
///
/// # Errors
/// The row could not be read, or does not read as a board.
pub fn load(db: &Connection, project: ProjectId) -> Result<Option<Board>, String> {
    let text: Option<String> = db
        .query_row(
            "SELECT board FROM boards WHERE project = ?1",
            [project],
            |row| row.get(0),
        )
        .optional()
        .map_err(|why| why.to_string())?;
    text.map(|text| {
        serde_json::from_str(&text)
            .map(settled)
            .map_err(|why| format!("the board of project {project} does not read: {why}"))
    })
    .transpose()
}

/// `board` with `next` past every `cN` it holds: a file edited by hand, or an old one, must not
/// give a new card an id another has.
fn settled(mut board: Board) -> Board {
    let used = (board.cards.iter())
        .filter_map(|card| card.id.strip_prefix('c')?.parse::<u32>().ok())
        .max();
    if let Some(used) = used {
        board.next = board.next.max(used.saturating_add(1));
    }
    board
}

/// `board` written as project `project`'s row, in `tx`.
///
/// # Errors
/// It did not write as JSON, or SQLite could not.
pub fn save(tx: &Transaction<'_>, project: ProjectId, board: &Board) -> Result<(), String> {
    let text = serde_json::to_string(board)
        .map_err(|why| format!("the board did not write as JSON: {why}"))?;
    tx.execute(
        "INSERT INTO boards (project, board, updated) VALUES (?1, ?2, ?3)
         ON CONFLICT (project) DO UPDATE SET board = excluded.board, updated = excluded.updated",
        params![project, text, super::now()],
    )
    .map(drop)
    .map_err(|why| why.to_string())
}

/// Every project with a board: its id and folder.
///
/// # Errors
/// SQLite could not read them.
pub fn projects(db: &Connection) -> Result<Vec<(ProjectId, PathBuf)>, String> {
    let mut rows = db
        .prepare_cached(
            "SELECT projects.id, projects.path FROM boards JOIN projects ON projects.id = boards.project
             ORDER BY projects.id",
        )
        .map_err(|why| why.to_string())?;
    let found = rows
        .query_map([], |row| {
            let path: String = row.get(1)?;
            Ok((row.get(0)?, PathBuf::from(path)))
        })
        .map_err(|why| why.to_string())?
        .filter_map(|row| {
            row.map_err(|why| log::warn!("board: a project's row did not read: {why}"))
                .ok()
        })
        .collect();
    Ok(found)
}

/// The board as the file of the project at `project` holds it: the desk's last export, or what
/// the project kept before the database. None where there is no file.
///
/// # Errors
/// The file could not be read, or does not read as a board: it is left as it is.
pub fn written(project: &Path) -> Result<Option<Board>, String> {
    let path = place(project);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(why) => return Err(format!("{} could not be read: {why}", path.display())),
    };
    serde_json::from_str(&text)
        .map(settled)
        .map(Some)
        .map_err(|why| {
            format!(
                "{} does not read as a board ({why}): fix it or move it away; it is left as it is",
                path.display()
            )
        })
}

/// `board` written out to the file of the project at `project`, whole beside it and renamed over
/// it so a reader never finds half a board. A project whose folder is gone (moved, deleted, on a
/// volume not mounted) is left as it is: its folder is not made again for a file. Nor is a board
/// written through a `.localix` that leads out of the project, as a cloned repository may ship.
///
/// # Errors
/// The file could not be written.
pub fn export(project: &Path, board: &Board) -> Result<(), String> {
    use std::io::Write as _;

    if !project.is_dir() {
        log::warn!(
            "board: {} is not a folder: its board file is not written",
            project.display()
        );
        return Ok(());
    }
    let path = place(project);
    let failed = |why: std::io::Error| format!("{} was not written: {why}", path.display());
    let dir = project.join(".localix");
    // The project is a folder now, so this makes `.localix` alone.
    std::fs::create_dir_all(&dir).map_err(failed)?;
    let dir = std::fs::canonicalize(&dir).map_err(failed)?;
    if !dir.starts_with(std::fs::canonicalize(project).map_err(failed)?) {
        return Err(format!(
            "{} leaves the project: its board is not written there",
            path.display()
        ));
    }
    let mut text = serde_json::to_string_pretty(board)
        .map_err(|why| format!("the board did not write as JSON: {why}"))?;
    text.push('\n');
    let path = dir.join("board.json");
    // A file that already says it is not written again: a watcher of the project sees no change.
    if std::fs::read_to_string(&path).is_ok_and(|now| now == text) {
        return Ok(());
    }
    let beside = dir.join(format!("board.json.{}", crate::token()));
    let wrote = (std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&beside))
    .and_then(|mut file| file.write_all(text.as_bytes()))
    .and_then(|()| std::fs::rename(&beside, &path));
    wrote.map_err(|why| {
        let _ = std::fs::remove_file(&beside);
        failed(why)
    })
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
