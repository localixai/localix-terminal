//! What a session's documents need besides joining the one they were asked from (beside.rs):
//! the session a document asked from none joins; the documents a project with no terminal
//! keeps in one tab, which its first session takes; a session's documents closed with it; and
//! the tabs of a run before brought to this shape as the app starts. A project with a session
//! keeps no tab of documents that is not one.

use std::sync::Arc;

use super::beside::{Beside, document, is, takes};
use super::layout::told;
use super::{Call, Desk};
use crate::feed::{Entity, Write};
use crate::layout::{self, Share, Tree, WindowId, WorkspaceId};
use crate::ledger::{Kind, Row};
use crate::projects::ProjectId;

impl Desk {
    /// Document `pane` of `project`, asked from no session (or from a tab that takes none),
    /// opened in `window` (none: the first): it joins a session of its workspace
    /// (`session_in`), as `person`'s or an agent's, and the panes that session's tab had are
    /// the answer. Where the project has no terminal, none: the document is the last of the
    /// ones its workspace keeps in one tab, and the one in sight there. Nothing when it is in a
    /// tab already (one kept across a restart).
    pub(super) fn place_loose(
        write: &Write<'_, '_>,
        pane: Row,
        (project, window): (ProjectId, Option<WindowId>),
        person: bool,
        now: u64,
    ) -> Result<Option<Vec<Row>>, String> {
        if layout::tab_of(write.tx(), pane)?.is_some() {
            return Ok(None);
        }
        let workspace = Self::workspace_for(write, project, now, window)?;
        if let Some(anchor) = session_in(write.tx(), workspace)? {
            let joined = Self::place_beside(write, pane, Beside { anchor, person }, now)?;
            if joined.is_some() {
                return Ok(joined);
            }
        }
        let loose = (layout::tabs(write.tx(), workspace)?.into_iter())
            .find(|tab| matches!(tab.tree, Tree::Stack { .. }));
        let Some(tab) = loose else {
            let tree = Tree::stack_of(pane);
            layout::open_tab(write.tx(), workspace, &tree, None, None, now)?;
            told(write, &[Entity::Workspace { id: workspace }])?;
            return Ok(None);
        };
        let mut tree = tab.tree;
        tree.stack(pane);
        layout::set_tree(write.tx(), tab.id, &tree, Some(pane))?;
        let changed = [
            Entity::Tab { id: tab.id },
            Entity::Workspace { id: workspace },
        ];
        told(write, &changed)?;
        Ok(None)
    }

    /// Terminal `pane`, just placed in a tab of its own, takes the documents its workspace kept
    /// without a session: it is one now.
    pub(super) fn takes_loose(write: &Write<'_, '_>, pane: Row, now: u64) -> Result<(), String> {
        let Some(id) = layout::tab_of(write.tx(), pane)? else {
            return Ok(());
        };
        let Some(tab) = layout::tab(write.tx(), id)? else {
            return Ok(());
        };
        join_loose(write, tab.workspace, pane, now).map(drop)
    }

    /// The session a document `call` asks from no session would join, as the layout is now.
    pub(super) fn session_for(&self, call: &Call) -> Option<Beside> {
        let wanted = call.window.or_else(|| self.in_use());
        let project = call.project.id;
        let anchor = (self.feed.store())
            .read(|db| {
                let first = layout::windows(db)?.first().map(|window| window.id);
                let Some(window) = wanted.or(first) else {
                    return Ok(None);
                };
                let space = (layout::workspaces(db, window)?.into_iter())
                    .find(|space| space.project == project);
                match space {
                    Some(space) => session_in(db, space.id),
                    None => Ok(None),
                }
            })
            .ok()??;
        Some(Beside {
            anchor,
            person: call.person,
        })
    }

    /// The documents of the session terminal `row` is, read as it closes: they close with it.
    pub(super) fn documents_of(&self, key: &str, row: Row) -> Vec<Row> {
        if !key.starts_with('t') {
            return vec![];
        }
        (self.feed.store())
            .read(|db| {
                let Some(id) = layout::tab_of(db, row)? else {
                    return Ok(vec![]);
                };
                Ok(layout::tab(db, id)?.map_or_else(Vec::new, |tab| tab.tree.stacked()))
            })
            .unwrap_or_default()
    }

    /// A closed session's `documents` closed after it, off the thread that closed it: each as
    /// its own agent's close would (a file's unsaved text stays its draft, a page its
    /// project's history).
    pub(super) fn close_documents(&self, documents: &[Row]) {
        let keys: Vec<String> = {
            let ledger = crate::lock(&self.ledger);
            (documents.iter())
                .filter_map(|row| ledger.iter().find(|(_, (kept, _))| kept == row))
                .map(|(key, _)| key.clone())
                .collect()
        };
        let Some(desk) = self.me.upgrade().filter(|_| !keys.is_empty()) else {
            return;
        };
        let closing = std::thread::Builder::new()
            .name("desk-session-closed".into())
            .spawn(move || close_each(&desk, &keys));
        if let Err(why) = closing {
            log::warn!("a closed session's documents stay open (no thread: {why})");
        }
    }

    /// The tabs a run before left, as sessions keep them: a document beside a terminal is that
    /// terminal's first document, on the side it stood, and the ones alone in a tab join a
    /// session of their workspace, or are one tab where it has none.
    pub(super) fn sessions_shaped(&self) {
        let shaped = self.feed.write(|write| {
            for window in layout::windows(write.tx())? {
                for space in layout::workspaces(write.tx(), window.id)? {
                    if shape(write, space.id, space.shown)? {
                        told(write, &[Entity::Workspace { id: space.id }])?;
                    }
                    if let Some(session) = session_in(write.tx(), space.id)? {
                        join_loose(write, space.id, session, crate::board::now())?;
                    }
                }
            }
            Ok(())
        });
        if let Err(why) = shaped {
            log::warn!("the tabs of the run before keep their shape: {why}");
        }
    }
}

/// The session of workspace `workspace` that a document asked from no session joins, by its
/// terminal: the tab the workspace shows when that is a session, else the last of its sessions.
/// None while the project has no terminal there.
fn session_in(
    db: &crate::store::Connection,
    workspace: WorkspaceId,
) -> Result<Option<Row>, String> {
    let shown = layout::workspace(db, workspace)?.and_then(|space| space.shown);
    let mut last = None;
    for tab in layout::tabs(db, workspace)? {
        let Some(terminal) = terminal_of(db, &tab.tree)? else {
            continue;
        };
        if Some(tab.id) == shown {
            return Ok(Some(terminal));
        }
        last = Some(terminal);
    }
    Ok(last)
}

/// The terminal a tab of `tree` is the session of: the pane documents join it by. None for
/// documents alone, the board, and panes the person put side by side.
fn terminal_of(db: &crate::store::Connection, tree: &Tree) -> Result<Option<Row>, String> {
    if matches!(tree, Tree::Stack { .. }) {
        return Ok(None);
    }
    for pane in tree.panes() {
        if is(db, pane, |kind| kind == Kind::Terminal)? && takes(db, tree, pane)? {
            return Ok(Some(pane));
        }
    }
    Ok(None)
}

/// The documents workspace `workspace` keeps without a session joined to the session of
/// terminal `anchor`: after its own, in their order, the one that was in sight in sight, and
/// that session shown where they were. Whether there were any.
fn join_loose(
    write: &Write<'_, '_>,
    workspace: WorkspaceId,
    anchor: Row,
    now: u64,
) -> Result<bool, String> {
    let tx = write.tx();
    let loose: Vec<layout::Tab> = (layout::tabs(tx, workspace)?.into_iter())
        .filter(|tab| matches!(tab.tree, Tree::Stack { .. }))
        .collect();
    let session = match layout::tab_of(tx, anchor)? {
        Some(id) => layout::tab(tx, id)?,
        None => None,
    };
    let Some(session) = session.filter(|_| !loose.is_empty()) else {
        return Ok(false);
    };
    if !takes(tx, &session.tree, anchor)? {
        return Ok(false);
    }
    let shown = layout::workspace(tx, workspace)?.and_then(|space| space.shown);
    let was_shown = loose.iter().any(|tab| Some(tab.id) == shown);
    let mut tree = session.tree.clone();
    let mut sight = None;
    for tab in &loose {
        if let Tree::Stack { panes, shown: seen } = &tab.tree {
            if sight.is_none() || Some(tab.id) == shown {
                sight = Some(*seen);
            }
            layout::close_tab(tx, tab.id, now)?;
            for pane in panes {
                tree.stack(*pane);
            }
        }
    }
    if let Some(sight) = sight {
        tree.show(sight);
    }
    layout::set_tree(tx, session.id, &tree, session.focused.or(Some(anchor)))?;
    if was_shown {
        layout::show_tab(tx, workspace, Some(session.id))?;
    }
    let changed = [
        Entity::Tab { id: session.id },
        Entity::Workspace { id: workspace },
    ];
    told(write, &changed)?;
    Ok(true)
}

/// Each of `keys` closed, a refusal logged: the session is gone either way.
fn close_each(desk: &Arc<Desk>, keys: &[String]) {
    for key in keys {
        if let Err(why) = desk.close_tab(key) {
            log::debug!("{key}, a closed session's document, did not close: {why}");
        }
    }
}

/// Workspace `workspace`'s tabs shaped as sessions keep them (`Desk::sessions_shaped`), `shown`
/// being the tab it shows. Whether anything changed.
fn shape(
    write: &Write<'_, '_>,
    workspace: WorkspaceId,
    shown: Option<layout::TabId>,
) -> Result<bool, String> {
    let tx = write.tx();
    let now = crate::board::now();
    let mut changed = false;
    let mut alone: Vec<(layout::TabId, Row)> = vec![];
    for tab in layout::tabs(tx, workspace)? {
        match &tab.tree {
            Tree::Leaf { pane } if is(tx, *pane, document)? => alone.push((tab.id, *pane)),
            Tree::Branch {
                across: true,
                children,
            } => {
                let [one, other] = children.as_slice() else {
                    continue;
                };
                let (Tree::Leaf { pane: first }, Tree::Leaf { pane: second }) =
                    (&one.tree, &other.tree)
                else {
                    continue;
                };
                let terminal = |pane: Row| is(tx, pane, |kind| kind == Kind::Terminal);
                let (doc, session) = match (is(tx, *first, document)?, is(tx, *second, document)?) {
                    (true, false) if terminal(*second)? => (one, other),
                    (false, true) if terminal(*first)? => (other, one),
                    _ => continue,
                };
                let Some(pane) = doc.tree.panes().first().copied() else {
                    continue;
                };
                let stack = Share {
                    share: doc.share,
                    tree: Tree::stack_of(pane),
                };
                let children = match std::ptr::eq(doc, one) {
                    true => vec![stack, session.clone()],
                    false => vec![session.clone(), stack],
                };
                let tree = Tree::Branch {
                    across: true,
                    children,
                };
                layout::set_tree(tx, tab.id, &tree, tab.focused)?;
                changed = true;
            }
            Tree::Leaf { .. } | Tree::Stack { .. } | Tree::Branch { .. } => {}
        }
    }
    let Some((first, _)) = alone.first() else {
        return Ok(changed);
    };
    // One tab for them all: the one shown among them stays the one in sight.
    let kept = (alone.iter().find(|(id, _)| Some(*id) == shown)).map_or(*first, |(id, _)| *id);
    let in_sight = alone
        .iter()
        .find(|(id, _)| *id == kept)
        .map(|(_, pane)| *pane);
    for (id, _) in &alone {
        if *id != kept {
            layout::close_tab(tx, *id, now)?;
        }
    }
    let panes: Vec<Row> = alone.iter().map(|(_, pane)| *pane).collect();
    let Some(in_sight) = in_sight else {
        return Ok(changed);
    };
    let tree = Tree::Stack {
        panes,
        shown: in_sight,
    };
    layout::set_tree(tx, kept, &tree, Some(in_sight))?;
    if shown.is_some_and(|shown| alone.iter().any(|(id, _)| *id == shown)) {
        layout::show_tab(tx, workspace, Some(kept))?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
