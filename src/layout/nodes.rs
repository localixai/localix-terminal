//! A tab's tree as rows (`pane_nodes`): written whole when it changes, read back a tab at a time
//! or, for a workspace, all its tabs' in one query.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Share, Tab, TabId, Tree, WorkspaceId, failed};
use crate::ledger::Row as PaneId;

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
    let sight = tree.visible();
    let focused = focused
        .filter(|pane| sight.contains(pane))
        .or_else(|| sight.first().copied());
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
        Tree::Stack { panes, shown } => {
            tx.execute(
                "INSERT INTO pane_nodes (tab, parent, place, share, across)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![tab, parent, place, share, STACKED],
            )
            .map_err(|why| failed(&why))?;
            let stack = tx.last_insert_rowid();
            for (place, pane) in panes.iter().enumerate() {
                // The one in sight has the stack's room, the rest none of it.
                let share = if pane == shown { 1.0 } else { 0.0 };
                tx.execute(
                    "INSERT INTO pane_nodes (tab, parent, place, share, pane, focused)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![tab, stack, place, share, pane, focused == Some(*pane)],
                )
                .map_err(|why| failed(&why))?;
            }
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

/// A stack's `across`: neither side by side (1) nor one above the other (0), but one over the
/// other.
const STACKED: i64 = 2;

/// One node as kept.
struct Node {
    id: i64,
    parent: Option<i64>,
    share: f64,
    across: Option<i64>,
    pane: Option<PaneId>,
    focused: bool,
}

/// A node from a row of `id, parent, share, across, pane, focused`, from column `from` on.
fn node(row: &rusqlite::Row<'_>, from: usize) -> rusqlite::Result<Node> {
    Ok(Node {
        id: row.get(from)?,
        parent: row.get(from + 1)?,
        share: row.get(from + 2)?,
        across: row.get(from + 3)?,
        pane: row.get(from + 4)?,
        focused: row.get(from + 5)?,
    })
}

/// A tab's tree and its pane in focus, from its nodes (in order of parent, then place); none
/// when it has no node.
fn tree_from(nodes: &[Node]) -> Option<(Tree, Option<PaneId>)> {
    let focused = (nodes.iter())
        .find(|node| node.focused)
        .and_then(|node| node.pane);
    let tree = (nodes.iter())
        .find(|node| node.parent.is_none())
        .and_then(|root| tree_of(nodes, root))?;
    Some((tree, focused))
}

fn tree_of(nodes: &[Node], node: &Node) -> Option<Tree> {
    match (node.pane, node.across) {
        (Some(pane), _) => Some(Tree::leaf(pane)),
        (None, Some(STACKED)) => {
            let kept = || nodes.iter().filter(|child| child.parent == Some(node.id));
            let panes: Vec<PaneId> = kept().filter_map(|child| child.pane).collect();
            let shown = kept()
                .find(|child| child.share > 0.0)
                .and_then(|child| child.pane);
            let shown = shown.or_else(|| panes.first().copied())?;
            Some(Tree::Stack { panes, shown })
        }
        (None, Some(across)) => {
            let across = across == 1;
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
    let row: Option<(WorkspaceId, usize, bool)> = db
        .query_row(
            "SELECT workspace, place, whole FROM tabs WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|why| failed(&why))?;
    let Some((workspace, place, whole)) = row else {
        return Ok(None);
    };
    let nodes = db
        .prepare_cached(
            "SELECT id, parent, share, across, pane, focused FROM pane_nodes WHERE tab = ?1
             ORDER BY parent, place, id",
        )
        .map_err(|why| failed(&why))?
        .query_map([id], |row| node(row, 0))
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    Ok(tree_from(&nodes).map(|(tree, focused)| Tab {
        id,
        workspace,
        place,
        tree,
        focused,
        whole,
    }))
}

/// Tab `id` shows its pane in focus alone (`whole`), or all it has in sight.
///
/// # Errors
/// The store refused it.
pub fn set_whole(tx: &Transaction<'_>, id: TabId, whole: bool) -> Result<(), String> {
    tx.execute(
        "UPDATE tabs SET whole = ?2 WHERE id = ?1",
        params![id, whole],
    )
    .map(drop)
    .map_err(|why| failed(&why))
}

/// Workspace `workspace`'s open tabs, in their places, each with its tree: two queries, however
/// many tabs there are.
///
/// # Errors
/// The store refused it.
pub fn tabs(db: &Connection, workspace: WorkspaceId) -> Result<Vec<Tab>, String> {
    let places = db
        .prepare_cached(
            "SELECT id, place, whole FROM tabs WHERE workspace = ?1 AND closed IS NULL
             ORDER BY place, id",
        )
        .map_err(|why| failed(&why))?
        .query_map([workspace], |row| {
            Ok((
                row.get::<_, TabId>(0)?,
                row.get::<_, usize>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })
        .map_err(|why| failed(&why))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|why| failed(&why))?;
    let mut nodes: HashMap<TabId, Vec<Node>> = HashMap::new();
    let mut query = db
        .prepare_cached(
            "SELECT tab, id, parent, share, across, pane, focused FROM pane_nodes
             WHERE tab IN (SELECT id FROM tabs WHERE workspace = ?1 AND closed IS NULL)
             ORDER BY tab, parent, place, id",
        )
        .map_err(|why| failed(&why))?;
    let read = query
        .query_map([workspace], |row| {
            Ok((row.get::<_, TabId>(0)?, node(row, 1)?))
        })
        .map_err(|why| failed(&why))?;
    for found in read {
        let (tab, node) = found.map_err(|why| failed(&why))?;
        nodes.entry(tab).or_default().push(node);
    }
    Ok(places
        .into_iter()
        .filter_map(|(id, place, whole)| {
            let (tree, focused) = tree_from(nodes.get(&id)?)?;
            Some(Tab {
                id,
                workspace,
                place,
                tree,
                focused,
                whole,
            })
        })
        .collect())
}
