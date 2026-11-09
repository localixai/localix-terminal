//! The layout, kept in the store: the windows, each window's workspaces (a window × a project),
//! each workspace's tabs in their places, and each tab's tree of panes (docs/windows.md). The
//! panes themselves are the ledger's rows (ledger.rs); a tree only names them. The desk writes it
//! (desk/layout.rs), each change with its notice; nothing here knows the window.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::ledger::Row as PaneId;
use crate::projects::ProjectId;

mod nodes;
mod tree;

pub use nodes::{set_tree, tab, tabs};
pub use tree::{LEAST, Share, Tree};

pub type WindowId = i64;
pub type WorkspaceId = i64;
pub type TabId = i64;

/// A window as kept: the workspace it shows. (Its frame and its side are the browser's, kept
/// with the person's settings.)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Window {
    pub id: WindowId,
    pub shown: Option<WorkspaceId>,
}

/// A window's tabs of one project, and the one it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub window: WindowId,
    pub project: ProjectId,
    pub shown: Option<TabId>,
}

/// A tab: its place in its workspace, its panes and the one in focus.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tab {
    pub id: TabId,
    pub workspace: WorkspaceId,
    pub place: usize,
    pub tree: Tree,
    pub focused: Option<PaneId>,
}

/// What the store said, as the layout's callers take it.
fn failed(why: &rusqlite::Error) -> String {
    why.to_string()
}

/// A window opened `now`: its row.
///
/// # Errors
/// The store refused it.
pub fn open_window(tx: &Transaction<'_>, now: u64) -> Result<WindowId, String> {
    tx.execute("INSERT INTO windows (opened) VALUES (?1)", [now])
        .map_err(|why| failed(&why))?;
    Ok(tx.last_insert_rowid())
}

/// How long a closed tab or window is kept before it is deleted, in seconds: a day. Its row only
/// stands for a late notice that names it; its panes are in the ledger.
const CLOSED_KEPT: u64 = 24 * 3600;

/// The tabs and windows closed more than [`CLOSED_KEPT`] before `now` deleted, with their
/// workspaces and trees (they go with them).
fn prune_closed(tx: &Transaction<'_>, now: u64) -> Result<(), String> {
    let before = now.saturating_sub(CLOSED_KEPT);
    tx.execute("DELETE FROM tabs WHERE closed < ?1", [before])
        .map_err(|why| failed(&why))?;
    tx.execute("DELETE FROM windows WHERE closed < ?1", [before])
        .map_err(|why| failed(&why))?;
    Ok(())
}

/// Window `id` closed `now`, with its tabs.
///
/// # Errors
/// The store refused it.
pub fn close_window(tx: &Transaction<'_>, id: WindowId, now: u64) -> Result<(), String> {
    tx.execute(
        "UPDATE tabs SET closed = ?2 WHERE closed IS NULL
         AND workspace IN (SELECT id FROM workspaces WHERE window = ?1)",
        params![id, now],
    )
    .map_err(|why| failed(&why))?;
    tx.execute(
        "DELETE FROM pane_nodes WHERE tab IN (SELECT tabs.id FROM tabs JOIN workspaces
         ON workspaces.id = tabs.workspace WHERE workspaces.window = ?1)",
        [id],
    )
    .map_err(|why| failed(&why))?;
    tx.execute(
        "UPDATE windows SET closed = ?2 WHERE id = ?1 AND closed IS NULL",
        params![id, now],
    )
    .map_err(|why| failed(&why))?;
    prune_closed(tx, now)
}

const WINDOW: &str = "id, shown";

fn window_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Window> {
    Ok(Window {
        id: row.get(0)?,
        shown: row.get(1)?,
    })
}

/// The windows open, the first opened first.
///
/// # Errors
/// The store refused it.
pub fn windows(db: &Connection) -> Result<Vec<Window>, String> {
    let mut query = db
        .prepare_cached(&format!(
            "SELECT {WINDOW} FROM windows WHERE closed IS NULL ORDER BY id"
        ))
        .map_err(|why| failed(&why))?;
    let windows = query
        .query_map([], window_of)
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    Ok(windows)
}

/// Window `id`, open or not.
///
/// # Errors
/// The store refused it.
pub fn window(db: &Connection, id: WindowId) -> Result<Option<Window>, String> {
    db.query_row(
        &format!("SELECT {WINDOW} FROM windows WHERE id = ?1"),
        [id],
        window_of,
    )
    .optional()
    .map_err(|why| failed(&why))
}

/// Window `window`'s workspace of project `project`, made `now` when it has none.
///
/// # Errors
/// The store refused it.
pub fn workspace_of(
    tx: &Transaction<'_>,
    window: WindowId,
    project: ProjectId,
    now: u64,
) -> Result<WorkspaceId, String> {
    tx.execute(
        "INSERT INTO workspaces (window, project, opened) VALUES (?1, ?2, ?3)
         ON CONFLICT (window, project) DO NOTHING",
        params![window, project, now],
    )
    .map_err(|why| failed(&why))?;
    tx.query_row(
        "SELECT id FROM workspaces WHERE window = ?1 AND project = ?2",
        params![window, project],
        |row| row.get(0),
    )
    .map_err(|why| failed(&why))
}

/// Window `window` shows workspace `workspace`.
///
/// # Errors
/// The store refused it.
pub fn show_workspace(
    tx: &Transaction<'_>,
    window: WindowId,
    workspace: WorkspaceId,
) -> Result<(), String> {
    tx.execute(
        "UPDATE windows SET shown = ?2 WHERE id = ?1",
        params![window, workspace],
    )
    .map_err(|why| failed(&why))?;
    Ok(())
}

fn workspace_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        window: row.get(1)?,
        project: row.get(2)?,
        shown: row.get(3)?,
    })
}

/// Window `window`'s workspaces, the first made first.
///
/// # Errors
/// The store refused it.
pub fn workspaces(db: &Connection, window: WindowId) -> Result<Vec<Workspace>, String> {
    let mut query = db
        .prepare_cached(
            "SELECT id, window, project, shown FROM workspaces WHERE window = ?1 ORDER BY id",
        )
        .map_err(|why| failed(&why))?;
    let found = query
        .query_map([window], workspace_row)
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    Ok(found)
}

/// Workspace `id`.
///
/// # Errors
/// The store refused it.
pub fn workspace(db: &Connection, id: WorkspaceId) -> Result<Option<Workspace>, String> {
    db.query_row(
        "SELECT id, window, project, shown FROM workspaces WHERE id = ?1",
        [id],
        workspace_row,
    )
    .optional()
    .map_err(|why| failed(&why))
}

/// The open tabs of workspace `workspace`, in their places.
fn open_ids(db: &Connection, workspace: WorkspaceId) -> Result<Vec<TabId>, String> {
    let mut query = db
        .prepare_cached(
            "SELECT id FROM tabs WHERE workspace = ?1 AND closed IS NULL ORDER BY place, id",
        )
        .map_err(|why| failed(&why))?;
    let ids = query
        .query_map([workspace], |row| row.get(0))
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    Ok(ids)
}

/// Workspace `workspace`'s open tabs placed in the order of `ids`, from 0.
fn renumber(tx: &Transaction<'_>, ids: &[TabId]) -> Result<(), String> {
    let mut place_at = tx
        .prepare_cached("UPDATE tabs SET place = ?2 WHERE id = ?1 AND place <> ?2")
        .map_err(|why| failed(&why))?;
    for (place, id) in ids.iter().enumerate() {
        place_at
            .execute(params![id, place])
            .map_err(|why| failed(&why))?;
    }
    Ok(())
}

/// A tab of `tree` opened `now` in workspace `workspace`, at place `at` (the last when none or
/// past the end), `focused` in focus: its row.
///
/// # Errors
/// The store refused it.
pub fn open_tab(
    tx: &Transaction<'_>,
    workspace: WorkspaceId,
    tree: &Tree,
    focused: Option<PaneId>,
    at: Option<usize>,
    now: u64,
) -> Result<TabId, String> {
    tx.execute(
        "INSERT INTO tabs (workspace, place, opened) VALUES (?1, -1, ?2)",
        params![workspace, now],
    )
    .map_err(|why| failed(&why))?;
    let id = tx.last_insert_rowid();
    let mut ids: Vec<TabId> = open_ids(tx, workspace)?
        .into_iter()
        .filter(|other| *other != id)
        .collect();
    let at = at.unwrap_or(ids.len()).min(ids.len());
    ids.insert(at, id);
    renumber(tx, &ids)?;
    set_tree(tx, id, tree, focused)?;
    Ok(id)
}

/// The open tab pane `pane` is in, if one is.
///
/// # Errors
/// The store refused it.
pub fn tab_of(db: &Connection, pane: PaneId) -> Result<Option<TabId>, String> {
    db.query_row(
        "SELECT tabs.id FROM pane_nodes JOIN tabs ON tabs.id = pane_nodes.tab
         WHERE pane_nodes.pane = ?1 AND tabs.closed IS NULL",
        [pane],
        |row| row.get(0),
    )
    .optional()
    .map_err(|why| failed(&why))
}

/// Tab `id` to workspace `workspace` (its own or another), at place `at` of it.
///
/// # Errors
/// The store refused it, or there is no such tab.
pub fn move_tab(
    tx: &Transaction<'_>,
    id: TabId,
    workspace: WorkspaceId,
    at: usize,
) -> Result<(), String> {
    let from: WorkspaceId = tx
        .query_row("SELECT workspace FROM tabs WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .map_err(|why| failed(&why))?;
    if from != workspace {
        tx.execute(
            "UPDATE tabs SET workspace = ?2 WHERE id = ?1",
            params![id, workspace],
        )
        .map_err(|why| failed(&why))?;
        tx.execute(
            "UPDATE workspaces SET shown = NULL WHERE id = ?1 AND shown = ?2",
            params![from, id],
        )
        .map_err(|why| failed(&why))?;
        renumber(tx, &open_ids(tx, from)?)?;
    }
    let mut ids: Vec<TabId> = (open_ids(tx, workspace)?.into_iter())
        .filter(|other| *other != id)
        .collect();
    ids.insert(at.min(ids.len()), id);
    renumber(tx, &ids)
}

/// Window `from`'s tabs moved into window `into`, each into `into`'s workspace of its project
/// (made when it has none), after the tabs there; then `from` closes. The windows whose tabs
/// moved and the workspaces they went to.
///
/// # Errors
/// The store refused it.
pub fn gather(
    tx: &Transaction<'_>,
    from: WindowId,
    into: WindowId,
    now: u64,
) -> Result<Vec<WorkspaceId>, String> {
    let mut into_spaces = vec![];
    for space in workspaces(tx, from)? {
        let target = workspace_of(tx, into, space.project, now)?;
        for tab in tabs(tx, space.id)? {
            move_tab(tx, tab.id, target, usize::MAX)?;
        }
        into_spaces.push(target);
    }
    close_window(tx, from, now)?;
    Ok(into_spaces)
}

/// Tab `id` closed `now`: its panes are in no tab any more.
///
/// # Errors
/// The store refused it.
pub fn close_tab(tx: &Transaction<'_>, id: TabId, now: u64) -> Result<(), String> {
    tx.execute(
        "UPDATE tabs SET closed = ?2 WHERE id = ?1 AND closed IS NULL",
        params![id, now],
    )
    .map_err(|why| failed(&why))?;
    tx.execute("DELETE FROM pane_nodes WHERE tab = ?1", [id])
        .map_err(|why| failed(&why))?;
    tx.execute("UPDATE workspaces SET shown = NULL WHERE shown = ?1", [id])
        .map_err(|why| failed(&why))?;
    let workspace: Option<WorkspaceId> = tx
        .query_row("SELECT workspace FROM tabs WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|why| failed(&why))?;
    if let Some(workspace) = workspace {
        renumber(tx, &open_ids(tx, workspace)?)?;
    }
    prune_closed(tx, now)
}

/// Workspace `workspace` shows tab `tab` (none: none of its).
///
/// # Errors
/// The store refused it.
pub fn show_tab(
    tx: &Transaction<'_>,
    workspace: WorkspaceId,
    tab: Option<TabId>,
) -> Result<(), String> {
    tx.execute(
        "UPDATE workspaces SET shown = ?2 WHERE id = ?1",
        params![workspace, tab],
    )
    .map_err(|why| failed(&why))?;
    Ok(())
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
