//! The layout, kept in the store: the windows, each window's workspaces (a window × a project),
//! each workspace's tabs in their places, and each tab's tree of panes (docs/windows.md). The
//! panes themselves are the ledger's rows (ledger.rs); a tree only names them. The desk writes it
//! (desk/layout.rs), each change with its notice; nothing here knows the window.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::ledger::Row as PaneId;
use crate::projects::ProjectId;

pub type WindowId = i64;
pub type WorkspaceId = i64;
pub type TabId = i64;

/// A tab's panes: one alone, or a branch of trees side by side (`across`) or one above the
/// other, each with its share of the branch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Tree {
    Leaf { pane: PaneId },
    Branch { across: bool, children: Vec<Share> },
}

/// A child of a branch and its share of it (the shares of a branch add up to 1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Share {
    pub share: f64,
    pub tree: Tree,
}

/// The least share a pane keeps of its branch.
pub const LEAST: f64 = 0.1;

impl Tree {
    /// Pane `pane`, alone.
    pub const fn leaf(pane: PaneId) -> Self {
        Self::Leaf { pane }
    }

    /// Its panes, in order: left to right, top to bottom.
    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { pane } => vec![*pane],
            Self::Branch { children, .. } => (children.iter())
                .flat_map(|child| child.tree.panes())
                .collect(),
        }
    }

    /// Whether pane `pane` is one of its.
    pub fn holds(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf { pane: it } => *it == pane,
            Self::Branch { children, .. } => children.iter().any(|child| child.tree.holds(pane)),
        }
    }

    /// Pane `new` beside pane `at`: after it (right of it, or under it) or before. In a branch
    /// of the same way it is one more child, sharing `at`'s share; else `at` becomes a branch of
    /// the two, half each. Whether `at` is one of its.
    pub fn split(&mut self, at: PaneId, new: PaneId, across: bool, after: bool) -> bool {
        match self {
            Self::Leaf { pane } if *pane == at => {
                let (first, second) = if after { (at, new) } else { (new, at) };
                *self = Self::Branch {
                    across,
                    children: vec![
                        Share {
                            share: 0.5,
                            tree: Self::leaf(first),
                        },
                        Share {
                            share: 0.5,
                            tree: Self::leaf(second),
                        },
                    ],
                };
                true
            }
            Self::Leaf { .. } => false,
            Self::Branch {
                across: way,
                children,
            } => {
                let place = (children.iter()).position(|child| child.tree == Self::leaf(at));
                if let (Some(place), true) = (place, *way == across) {
                    let half = children[place].share / 2.0;
                    children[place].share = half;
                    let at_place = if after { place + 1 } else { place };
                    children.insert(
                        at_place,
                        Share {
                            share: half,
                            tree: Self::leaf(new),
                        },
                    );
                    return true;
                }
                (children.iter_mut()).any(|child| child.tree.split(at, new, across, after))
            }
        }
    }

    /// The tree without pane `pane`: its share goes to the rest of its branch, and a branch left
    /// with one child is that child. None when `pane` was all of it.
    pub fn without(self, pane: PaneId) -> Option<Self> {
        match self {
            Self::Leaf { pane: it } => (it != pane).then_some(self),
            Self::Branch { across, children } => {
                let mut kept: Vec<Share> = (children.into_iter())
                    .filter_map(|child| {
                        let share = child.share;
                        child.tree.without(pane).map(|tree| Share { share, tree })
                    })
                    .collect();
                match kept.len() {
                    0 => None,
                    1 => kept.pop().map(|child| child.tree),
                    _ => {
                        let whole: f64 = kept.iter().map(|child| child.share).sum();
                        for child in &mut kept {
                            child.share = if whole > 0.0 {
                                child.share / whole
                            } else {
                                1.0 / 2.0
                            };
                        }
                        Some(Self::Branch {
                            across,
                            children: kept,
                        })
                    }
                }
            }
        }
    }

    /// The branch at `path` (the place of each child down from the root) gives its first `at`
    /// children `share` of it, the rest the remainder, each side keeping its proportions, and
    /// no child under `LEAST`. Whether there is such a branch.
    pub fn resize(&mut self, path: &[usize], at: usize, share: f64) -> bool {
        match (self, path.split_first()) {
            (Self::Branch { children, .. }, Some((&first, rest))) => {
                match children.get_mut(first) {
                    Some(child) => child.tree.resize(rest, at, share),
                    None => false,
                }
            }
            (Self::Branch { children, .. }, None) if at > 0 && at < children.len() => {
                let least = LEAST * at as f64;
                let most = 1.0 - LEAST * (children.len() - at) as f64;
                let share = share.clamp(least, most.max(least));
                let (before, after) = children.split_at_mut(at);
                spread(before, share);
                spread(after, 1.0 - share);
                true
            }
            _ => false,
        }
    }
}

/// `children` given `whole` between them, in their proportions now.
fn spread(children: &mut [Share], whole: f64) {
    let now: f64 = children.iter().map(|child| child.share).sum();
    let count = children.len() as f64;
    for child in children {
        child.share = if now > 0.0 {
            child.share / now * whole
        } else {
            whole / count
        };
    }
}

/// A window as kept: the workspace it shows, where it was and its side.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Window {
    pub id: WindowId,
    pub shown: Option<WorkspaceId>,
    pub frame: Option<Frame>,
    pub side_open: bool,
    pub side_width: Option<f64>,
}

/// Where a window is on the screen, in points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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
    Ok(())
}

/// Window `id`'s frame and side, as the person left them.
///
/// # Errors
/// The store refused it.
pub fn place_window(
    tx: &Transaction<'_>,
    id: WindowId,
    frame: Option<Frame>,
    side: (bool, Option<f64>),
) -> Result<(), String> {
    tx.execute(
        "UPDATE windows SET x = ?2, y = ?3, width = ?4, height = ?5, side_open = ?6,
         side_width = ?7 WHERE id = ?1",
        params![
            id,
            frame.map(|frame| frame.x),
            frame.map(|frame| frame.y),
            frame.map(|frame| frame.width),
            frame.map(|frame| frame.height),
            side.0,
            side.1,
        ],
    )
    .map_err(|why| failed(&why))?;
    Ok(())
}

const WINDOW: &str = "id, shown, x, y, width, height, side_open, side_width";

fn window_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Window> {
    let (x, y, width, height): (Option<f64>, Option<f64>, Option<f64>, Option<f64>) =
        (row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?);
    Ok(Window {
        id: row.get(0)?,
        shown: row.get(1)?,
        frame: match (x, y, width, height) {
            (Some(x), Some(y), Some(width), Some(height)) => Some(Frame {
                x,
                y,
                width,
                height,
            }),
            _ => None,
        },
        side_open: row.get(6)?,
        side_width: row.get(7)?,
    })
}

/// The windows open, the first opened first.
///
/// # Errors
/// The store refused it.
pub fn windows(db: &Connection) -> Result<Vec<Window>, String> {
    let mut query = db
        .prepare(&format!(
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
        .prepare("SELECT id, window, project, shown FROM workspaces WHERE window = ?1 ORDER BY id")
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
        .prepare("SELECT id FROM tabs WHERE workspace = ?1 AND closed IS NULL ORDER BY place, id")
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
    for (place, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE tabs SET place = ?2 WHERE id = ?1 AND place <> ?2",
            params![id, place],
        )
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

/// Tab `id`'s panes are `tree` now, `focused` in focus.
///
/// # Errors
/// The store refused it.
pub fn set_tree(
    tx: &Transaction<'_>,
    id: TabId,
    tree: &Tree,
    focused: Option<PaneId>,
) -> Result<(), String> {
    tx.execute("DELETE FROM pane_nodes WHERE tab = ?1", [id])
        .map_err(|why| failed(&why))?;
    let focused = focused
        .filter(|pane| tree.holds(*pane))
        .or_else(|| tree.panes().first().copied());
    write_node(tx, id, None, 0, 1.0, tree, focused)
}

fn write_node(
    tx: &Transaction<'_>,
    tab: TabId,
    parent: Option<i64>,
    place: usize,
    share: f64,
    tree: &Tree,
    focused: Option<PaneId>,
) -> Result<(), String> {
    match tree {
        Tree::Leaf { pane } => {
            tx.execute(
                "INSERT INTO pane_nodes (tab, parent, place, share, pane, focused)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![tab, parent, place, share, pane, focused == Some(*pane)],
            )
            .map_err(|why| failed(&why))?;
        }
        Tree::Branch { across, children } => {
            tx.execute(
                "INSERT INTO pane_nodes (tab, parent, place, share, across)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![tab, parent, place, share, across],
            )
            .map_err(|why| failed(&why))?;
            let branch = tx.last_insert_rowid();
            for (place, child) in children.iter().enumerate() {
                write_node(
                    tx,
                    tab,
                    Some(branch),
                    place,
                    child.share,
                    &child.tree,
                    focused,
                )?;
            }
        }
    }
    Ok(())
}

/// One node as kept.
struct Node {
    id: i64,
    parent: Option<i64>,
    share: f64,
    across: Option<bool>,
    pane: Option<PaneId>,
    focused: bool,
}

/// Tab `id`'s tree as kept, and its pane in focus; none when it has no node.
fn read_tree(db: &Connection, id: TabId) -> Result<Option<(Tree, Option<PaneId>)>, String> {
    let mut query = db
        .prepare(
            "SELECT id, parent, share, across, pane, focused FROM pane_nodes WHERE tab = ?1
             ORDER BY parent, place, id",
        )
        .map_err(|why| failed(&why))?;
    let nodes = query
        .query_map([id], |row| {
            Ok(Node {
                id: row.get(0)?,
                parent: row.get(1)?,
                share: row.get(2)?,
                across: row.get(3)?,
                pane: row.get(4)?,
                focused: row.get(5)?,
            })
        })
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    let focused = (nodes.iter())
        .find(|node| node.focused)
        .and_then(|node| node.pane);
    let tree = (nodes.iter())
        .find(|node| node.parent.is_none())
        .and_then(|root| tree_of(&nodes, root));
    Ok(tree.map(|tree| (tree, focused)))
}

fn tree_of(nodes: &[Node], node: &Node) -> Option<Tree> {
    match (node.pane, node.across) {
        (Some(pane), _) => Some(Tree::leaf(pane)),
        (None, Some(across)) => {
            let children: Vec<Share> = (nodes.iter())
                .filter(|child| child.parent == Some(node.id))
                .filter_map(|child| {
                    let tree = tree_of(nodes, child)?;
                    Some(Share {
                        share: child.share,
                        tree,
                    })
                })
                .collect();
            match children.len() {
                0 => None,
                1 => children.into_iter().next().map(|child| child.tree),
                _ => Some(Tree::Branch { across, children }),
            }
        }
        (None, None) => None,
    }
}

/// Tab `id`, open or not, with its tree; none when it is not, or has no pane.
///
/// # Errors
/// The store refused it.
pub fn tab(db: &Connection, id: TabId) -> Result<Option<Tab>, String> {
    let row: Option<(WorkspaceId, usize)> = db
        .query_row(
            "SELECT workspace, place FROM tabs WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|why| failed(&why))?;
    let Some((workspace, place)) = row else {
        return Ok(None);
    };
    Ok(read_tree(db, id)?.map(|(tree, focused)| Tab {
        id,
        workspace,
        place,
        tree,
        focused,
    }))
}

/// Workspace `workspace`'s open tabs, in their places, each with its tree.
///
/// # Errors
/// The store refused it.
pub fn tabs(db: &Connection, workspace: WorkspaceId) -> Result<Vec<Tab>, String> {
    let mut out = vec![];
    for id in open_ids(db, workspace)? {
        if let Some(tab) = tab(db, id)? {
            out.push(tab);
        }
    }
    Ok(out)
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
    Ok(())
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
