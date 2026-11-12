//! A split told of a session and a document (docs/windows.md): never a pair of panes side by
//! side as two terminals are, since a session keeps its documents in a stack. The document is
//! shown in its session, or joins the session it was put beside; and a session made whole puts
//! its documents by without letting go of them.

use super::Desk;
use super::beside::{document, is, takes};
use super::layout::told;
use crate::feed::{Entity, Write};
use crate::layout;
use crate::ledger::Row;

impl Desk {
    /// The split of `shown` and `beside` kept as a session keeps it, `shown` the pane in use
    /// and the one of them on the left as `right` says. Whether it was one: of one tab, a
    /// document of its stack in sight and the tab no longer whole; of two, a document joining
    /// the session beside it. A session's tab takes no other pane, and does not leave its
    /// documents for another tab: that is refused.
    pub(super) fn split_session(
        write: &Write<'_, '_>,
        (shown, beside): (Row, Row),
        right: bool,
        now: u64,
    ) -> Result<bool, String> {
        let tx = write.tx();
        let tab_of = |pane: Row| -> Result<Option<layout::Tab>, String> {
            match layout::tab_of(tx, pane)? {
                Some(id) => layout::tab(tx, id),
                None => Ok(None),
            }
        };
        let (Some(mine), Some(other)) = (tab_of(shown)?, tab_of(beside)?) else {
            return Ok(false);
        };
        let left = if right { shown } else { beside };
        if mine.id == other.id {
            let stacked = mine.tree.stacked();
            let Some(doc) = [shown, beside]
                .into_iter()
                .find(|pane| stacked.contains(pane))
            else {
                return Ok(false);
            };
            let mut tree = mine.tree.clone();
            tree.show(doc);
            // Two documents of one stack are never side by side: the one in use is in sight.
            if stacked.contains(&shown) {
                tree.show(shown);
            }
            tree.order(left);
            if tree != mine.tree || mine.focused != Some(shown) {
                layout::set_tree(tx, mine.id, &tree, Some(shown))?;
            }
            layout::set_whole(tx, mine.id, false)?;
            told(write, &[Entity::Tab { id: mine.id }])?;
            return Ok(true);
        }
        // Which of the two is the document, and which the session it joins.
        let joins = if is(tx, beside, document)? && takes(tx, &mine.tree, shown)? {
            Some((beside, shown))
        } else if is(tx, shown, document)? && takes(tx, &other.tree, beside)? {
            Some((shown, beside))
        } else {
            None
        };
        let Some((doc, session)) = joins else {
            let kept = [&mine, &other]
                .iter()
                .any(|tab| !tab.tree.stacked().is_empty());
            return match kept {
                true => Err("a session keeps its documents, and takes no other pane".into()),
                false => Ok(false),
            };
        };
        Self::unplace_pane(write, doc, now)?;
        let Some(tab) = tab_of(session)? else {
            return Ok(false);
        };
        let mut tree = tab.tree.clone();
        if !tree.stack(doc) {
            return Ok(false);
        }
        tree.order(left);
        layout::set_tree(tx, tab.id, &tree, Some(shown))?;
        layout::set_whole(tx, tab.id, false)?;
        layout::show_tab(tx, tab.workspace, Some(tab.id))?;
        told(
            write,
            &[
                Entity::Tab { id: tab.id },
                Entity::Workspace { id: tab.workspace },
            ],
        )?;
        Ok(true)
    }

    /// The tab of `shown` and `beside` made whole when it is a session with its documents:
    /// both still open, it shows `shown` alone and keeps the rest put by. Whether the tab is
    /// such a session (one of the two gone, it is left as it is: the stack shows its next).
    pub(super) fn whole_session(
        write: &Write<'_, '_>,
        rows: [Option<Row>; 2],
    ) -> Result<bool, String> {
        let tx = write.tx();
        for row in rows.into_iter().flatten() {
            let Some(id) = layout::tab_of(tx, row)? else {
                continue;
            };
            let Some(tab) = layout::tab(tx, id)? else {
                continue;
            };
            if tab.tree.stacked().is_empty() {
                continue;
            }
            let panes = tab.tree.panes();
            if let [Some(shown), Some(beside)] = rows
                && panes.contains(&shown)
                && panes.contains(&beside)
            {
                let mut tree = tab.tree;
                tree.show(shown);
                layout::set_tree(tx, id, &tree, Some(shown))?;
                layout::set_whole(tx, id, true)?;
                told(write, &[Entity::Tab { id }])?;
            }
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "layout_split_tests.rs"]
mod tests;
