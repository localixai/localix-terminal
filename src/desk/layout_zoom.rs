//! A tab's zoom (docs/windows.md): one pane of a tab of several drawn alone over the tab's
//! part, the rest kept in its tree, out of sight and running, as a tab out of sight is. It is
//! the tab's `whole`: another pane of the tab shown takes the zoom; a new split, a pane closed
//! or taken out, a divider moved and the tab made even end it (layout.rs, layout_split.rs,
//! layout_resize.rs). The person zooms with ⇧⌘↩, an agent with `show` and `zoom`.

use std::sync::Arc;

use crate::feed::Entity;
use crate::layout;
use crate::ledger::Row;

use super::told;
use crate::desk::Desk;

impl Desk {
    /// Pane `pane` alone over its tab (`on`), its tab shown and the pane in focus; or its tab
    /// drawn whole again. Whether the tab has more than the pane in sight: a pane alone has
    /// nothing to put by, and nothing changes.
    pub(in crate::desk) fn zoom_pane(&self, pane: Row, on: bool) -> Result<bool, String> {
        self.feed.write(|write| {
            let tx = write.tx();
            let Some(id) = layout::tab_of(tx, pane)? else {
                return Ok(false);
            };
            let Some(tab) = layout::tab(tx, id)? else {
                return Ok(false);
            };
            let mut tree = tab.tree.clone();
            tree.show(&pane);
            if tree.visible().len() < 2 {
                return Ok(false);
            }
            let mut changed = vec![Entity::Tab { id }];
            let shown = layout::workspace(tx, tab.workspace)?.and_then(|space| space.shown);
            if shown != Some(id) {
                layout::show_tab(tx, tab.workspace, Some(id))?;
                changed.push(Entity::Workspace { id: tab.workspace });
            }
            layout::set_tree(tx, id, &tree, Some(pane))?;
            layout::set_whole(tx, id, on)?;
            told(write, &changed)?;
            Ok(true)
        })
    }

    /// The person zoomed pane `key` (⇧⌘↩, the Pane menu) or drew its tab whole again: whether
    /// there was something to put by.
    pub(in crate::desk) fn zoomed(&self, key: &str, on: bool) -> Result<bool, String> {
        let Some((pane, _)) = crate::lock(&self.ledger).get(key).copied() else {
            return Ok(false);
        };
        self.zoom_pane(pane, on)
    }

    /// The person zoomed pane `key` or drew its tab whole again, kept on the writer thread
    /// after what the window told before.
    pub(in crate::desk) fn zoomed_later(self: &Arc<Self>, key: &str, on: bool) {
        let (desk, key) = (Arc::clone(self), key.to_string());
        self.write_later(move || {
            if let Err(why) = desk.zoomed(&key, on) {
                log::warn!("the zoom is not in the layout: {why}");
            }
        });
    }

    /// An agent's `show` with `zoom`: pane `key` alone over its tab, or the tab whole again,
    /// in words.
    pub(in crate::desk) fn zoom_shown(&self, key: &str, on: bool) -> Result<String, String> {
        if !self.zoomed(key, on)? {
            return Ok(format!("{key} is alone in its tab: nothing to zoom."));
        }
        if !on {
            return Ok(format!("{key}'s tab shows all its panes."));
        }
        let row = crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let hidden = (row.and_then(|row| {
            let read = |db: &crate::store::Connection| {
                let tab = layout::tab_of(db, row)?;
                tab.map_or(Ok(None), |id| layout::tab(db, id))
            };
            self.feed.store().read(read).ok().flatten()
        }))
        .map_or(0, |tab| tab.tree.visible().len().saturating_sub(1));
        let panes = if hidden == 1 { "pane" } else { "panes" };
        Ok(format!("{key} fills its tab; {hidden} {panes} put by."))
    }
}

#[cfg(test)]
#[path = "layout_zoom_tests.rs"]
mod tests;
