//! The ledger of a project's panes, kept in the store: every pane the window opened for it (a
//! page, a terminal, the board), open or closed, and the pages it showed. A closed pane is the
//! project's history; the panes open when the app quit are the ones it opens again; the pages
//! shown are what the address field suggests. The desk writes it (desk/ledger.rs); nothing here
//! knows the window. Where a pane is shown (its tab) is the layout's (layout.rs).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::projects::ProjectId;

/// A pane's row.
pub type Row = i64;

/// How many closed panes a project keeps, and how many pages shown.
const CLOSED_KEPT: usize = 200;
const VISITS_KEPT: usize = 1000;

/// How many visits are kept between two trims of a project's oldest.
const TRIM_EVERY: i64 = 64;

/// What a pane is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Page,
    Terminal,
    Board,
    File,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Terminal => "terminal",
            Self::Board => "board",
            Self::File => "file",
        }
    }

    fn of(name: &str) -> Self {
        match name {
            "terminal" => Self::Terminal,
            "board" => Self::Board,
            "file" => Self::File,
            _ => Self::Page,
        }
    }
}

/// A pane as the ledger keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub row: Row,
    pub project: ProjectId,
    pub kind: Kind,
    /// The agent that opened it; none: the person, or the project's own agent.
    pub agent: Option<String>,
    pub title: String,
    /// A page's address.
    pub url: Option<String>,
    /// A terminal's folder, or the file an editor's tab holds; and the program a terminal was
    /// opened with.
    pub cwd: Option<PathBuf>,
    pub command: Option<String>,
    pub opened: u64,
    /// When it closed; none while it is open (or was, when the app quit).
    pub closed: Option<u64>,
}

/// A pane opened: what is known of it then.
#[derive(Debug, Clone, Copy)]
pub struct Opened<'a> {
    pub project: ProjectId,
    pub kind: Kind,
    pub agent: Option<&'a str>,
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub cwd: Option<&'a Path>,
    pub command: Option<&'a str>,
}

/// A page the project showed: what the address field suggests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visit {
    pub url: String,
    pub title: String,
}

const COLUMNS: &str = "id, project, kind, agent, title, url, cwd, command, opened, closed";

fn pane(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pane> {
    let kind: String = row.get(2)?;
    let cwd: Option<String> = row.get(6)?;
    Ok(Pane {
        row: row.get(0)?,
        project: row.get(1)?,
        kind: Kind::of(&kind),
        agent: row.get(3)?,
        title: row.get(4)?,
        url: row.get(5)?,
        cwd: cwd.map(PathBuf::from),
        command: row.get(7)?,
        opened: row.get(8)?,
        closed: row.get(9)?,
    })
}

/// What the store said, as the ledger's callers take it.
fn failed(why: &rusqlite::Error) -> String {
    why.to_string()
}

/// Pane `row`, left open by the run before, open again in this one: the same row, its history
/// and its place in its tab with it.
///
/// # Errors
/// The store refused it.
pub fn reopen(tx: &Transaction<'_>, row: Row) -> Result<(), String> {
    tx.execute("UPDATE panes SET closed = NULL WHERE id = ?1", [row])
        .map_err(|why| failed(&why))?;
    Ok(())
}

/// Pane `opened`, `now`: its row, last of its project's.
///
/// # Errors
/// The store refused it.
pub fn open(tx: &Transaction<'_>, opened: &Opened<'_>, now: u64) -> Result<Row, String> {
    let place: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(place), 0) + 1 FROM panes WHERE project = ?1",
            [opened.project],
            |row| row.get(0),
        )
        .map_err(|why| failed(&why))?;
    tx.execute(
        "INSERT INTO panes (project, kind, agent, title, url, cwd, command, place, opened, shown)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            opened.project,
            opened.kind.name(),
            opened.agent,
            opened.title,
            opened.url,
            opened.cwd.map(|cwd| cwd.display().to_string()),
            opened.command,
            place,
            now,
        ],
    )
    .map_err(|why| failed(&why))?;
    Ok(tx.last_insert_rowid())
}

/// Pane `row` is titled `title`; whether that is news.
///
/// # Errors
/// The store refused it.
pub fn titled(tx: &Transaction<'_>, row: Row, title: &str) -> Result<bool, String> {
    let changed = tx
        .execute(
            "UPDATE panes SET title = ?2 WHERE id = ?1 AND title <> ?2",
            params![row, title],
        )
        .map_err(|why| failed(&why))?;
    Ok(changed > 0)
}

/// File `row` is at `path` now, titled `title`: where it opens again from.
///
/// # Errors
/// The store refused it.
pub fn moved(tx: &Transaction<'_>, row: Row, title: &str, path: &Path) -> Result<(), String> {
    let (set, path) = (
        "UPDATE panes SET title = ?2, cwd = ?3 WHERE id = ?1",
        path.display(),
    );
    (tx.execute(set, params![row, title, path.to_string()])
        .map(drop))
    .map_err(|why| failed(&why))
}

/// Page `row` showed `url`, titled `title`, `now`: its address and title, and a visit of its
/// project's when it is a web page or a file; whether that is news.
///
/// # Errors
/// The store refused it.
pub fn went(
    tx: &Transaction<'_>,
    row: Row,
    url: &str,
    title: &str,
    now: u64,
) -> Result<bool, String> {
    let before: Option<(ProjectId, Option<String>, String)> = tx
        .query_row(
            "SELECT project, url, title FROM panes WHERE id = ?1",
            [row],
            |found| Ok((found.get(0)?, found.get(1)?, found.get(2)?)),
        )
        .optional()
        .map_err(|why| failed(&why))?;
    let Some((project, was, titled)) = before else {
        return Ok(false);
    };
    let moved = was.as_deref() != Some(url);
    if !moved && titled == title {
        return Ok(false);
    }
    tx.execute(
        "UPDATE panes SET url = ?2, title = ?3, shown = ?4 WHERE id = ?1",
        params![row, url, title, now],
    )
    .map_err(|why| failed(&why))?;
    if visited(url) {
        remember(tx, project, row, url, title, now, moved)?;
    }
    Ok(true)
}

/// Whether `url` is a page worth suggesting again: a web page or a file, not a blank one.
fn visited(url: &str) -> bool {
    ["http://", "https://", "file://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// The visit of `url` kept: a new one when the page `moved` to it, else the last one titled.
fn remember(
    tx: &Transaction<'_>,
    project: ProjectId,
    row: Row,
    url: &str,
    title: &str,
    now: u64,
    moved: bool,
) -> Result<(), String> {
    let retitled = match moved {
        true => 0,
        false => tx
            .execute(
                "UPDATE visits SET title = ?3 WHERE id =
                 (SELECT MAX(id) FROM visits WHERE pane = ?1 AND url = ?2)",
                params![row, url, title],
            )
            .map_err(|why| failed(&why))?,
    };
    if retitled == 0 {
        tx.prepare_cached(
            "INSERT INTO visits (project, pane, url, title, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .and_then(|mut insert| insert.execute(params![project, row, url, title, now]))
        .map_err(|why| failed(&why))?;
        // The oldest go now and then, not with every visit: `visits` reads the newest
        // `VISITS_KEPT` alone, so the few kept past them meanwhile are never suggested.
        if tx.last_insert_rowid() % TRIM_EVERY != 0 {
            return Ok(());
        }
        tx.execute(
            "DELETE FROM visits WHERE project = ?1 AND id NOT IN
             (SELECT id FROM visits WHERE project = ?1 ORDER BY id DESC LIMIT ?2)",
            params![project, VISITS_KEPT],
        )
        .map_err(|why| failed(&why))?;
    }
    Ok(())
}

/// Pane `row` closed `now`; its project keeps the newest closed panes only.
///
/// # Errors
/// The store refused it.
pub fn close(tx: &Transaction<'_>, row: Row, now: u64) -> Result<(), String> {
    tx.execute(
        "UPDATE panes SET closed = ?2 WHERE id = ?1 AND closed IS NULL",
        params![row, now],
    )
    .map_err(|why| failed(&why))?;
    tx.execute(
        "DELETE FROM panes WHERE closed IS NOT NULL
         AND project = (SELECT project FROM panes WHERE id = ?1)
         AND id NOT IN (SELECT id FROM panes WHERE closed IS NOT NULL
             AND project = (SELECT project FROM panes WHERE id = ?1)
             ORDER BY closed DESC, id DESC LIMIT ?2)",
        params![row, CLOSED_KEPT],
    )
    .map_err(|why| failed(&why))?;
    Ok(())
}

/// Pane `row` taken out of its project's ledger: it opened again, as a new pane (a closed one,
/// or one open when the app last quit).
///
/// # Errors
/// The store refused it.
pub fn forget(tx: &Transaction<'_>, row: Row) -> Result<(), String> {
    tx.execute("DELETE FROM panes WHERE id = ?1", [row])
        .map_err(|why| failed(&why))?;
    Ok(())
}

/// Terminal `row`'s screen, `text`, kept `now`: shown again when it is restored.
///
/// # Errors
/// The store refused it.
pub fn keep_screen(tx: &Transaction<'_>, row: Row, text: &str, now: u64) -> Result<(), String> {
    tx.execute(
        "INSERT INTO screens (pane, text, at) VALUES (?1, ?2, ?3)
         ON CONFLICT (pane) DO UPDATE SET text = excluded.text, at = excluded.at",
        params![row, text, now],
    )
    .map_err(|why| failed(&why))?;
    Ok(())
}

/// The screen terminal `row` showed last, when it was kept.
///
/// # Errors
/// The store could not be read.
pub fn screen(db: &Connection, row: Row) -> Result<Option<String>, String> {
    db.query_row("SELECT text FROM screens WHERE pane = ?1", [row], |found| {
        found.get(0)
    })
    .optional()
    .map_err(|why| failed(&why))
}

/// Pane `row`, while it is kept.
///
/// # Errors
/// The store could not be read.
pub fn get(db: &Connection, row: Row) -> Result<Option<Pane>, String> {
    db.query_row(
        &format!("SELECT {COLUMNS} FROM panes WHERE id = ?1"),
        [row],
        pane,
    )
    .optional()
    .map_err(|why| failed(&why))
}

/// Project `project`'s panes, the latest first (by when each closed, else when it last showed,
/// else when it opened), `most` of them.
///
/// # Errors
/// The store could not be read.
pub fn past(db: &Connection, project: ProjectId, most: usize) -> Result<Vec<Pane>, String> {
    let mut statement = db
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM panes WHERE project = ?1
             ORDER BY COALESCE(closed, shown, opened) DESC, id DESC LIMIT ?2"
        ))
        .map_err(|why| failed(&why))?;
    let rows = statement
        .query_map(params![project, most], pane)
        .map_err(|why| failed(&why))?;
    rows.collect::<Result<_, _>>().map_err(|why| failed(&why))
}

/// Project `project`'s panes not closed, in their order: the ones open when the app quit.
///
/// # Errors
/// The store could not be read.
pub fn left_open(db: &Connection, project: ProjectId) -> Result<Vec<Pane>, String> {
    let mut statement = db
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM panes WHERE project = ?1 AND closed IS NULL ORDER BY place"
        ))
        .map_err(|why| failed(&why))?;
    let rows = statement
        .query_map([project], pane)
        .map_err(|why| failed(&why))?;
    rows.collect::<Result<_, _>>().map_err(|why| failed(&why))
}

/// The pages project `project` showed, the latest first, each once: `most` of them.
///
/// # Errors
/// The store could not be read.
pub fn visits(db: &Connection, project: ProjectId, most: usize) -> Result<Vec<Visit>, String> {
    let mut statement = db
        .prepare_cached(
            "SELECT url, title FROM visits WHERE id IN
             (SELECT MAX(id) FROM (SELECT id, url FROM visits WHERE project = ?1
                 ORDER BY id DESC LIMIT ?3) GROUP BY url)
             ORDER BY id DESC LIMIT ?2",
        )
        .map_err(|why| failed(&why))?;
    let rows = statement
        .query_map(params![project, most, VISITS_KEPT], |row| {
            Ok(Visit {
                url: row.get(0)?,
                title: row.get(1)?,
            })
        })
        .map_err(|why| failed(&why))?;
    rows.collect::<Result<_, _>>().map_err(|why| failed(&why))
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
