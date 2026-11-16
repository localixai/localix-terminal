//! What the person does to a tab's tree, as the window tells it, kept in the layout in the same
//! transaction as its notices: a pane put beside another, one taken out, a divider's final
//! share, the tab made even (either ends the tab's zoom), and the pane in focus.

use std::sync::Arc;

use crate::feed::Entity;
use crate::layout;
use crate::layout::geometry::Side;
use crate::ledger::Row;

use super::Desk;
use super::layout::told;

impl Desk {
    /// The row of pane `key`, read at once as the window tells of it; none once the app is
    /// ending (its panes go, and the layout stays as it is for when they open again).
    fn row_told(&self, key: &str) -> Option<Row> {
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        crate::lock(&self.ledger).get(key).map(|(row, _)| *row)
    }

    /// The person put pane `key` on `side` of pane `beside`: kept on the writer thread after
    /// what the window told before.
    pub(super) fn split_later(self: &Arc<Self>, key: &str, beside: &str, side: Side) {
        if let (Some(pane), Some(at)) = (self.row_told(key), self.row_told(beside)) {
            let desk = Arc::clone(self);
            self.write_later(move || {
                if let Err(why) = desk.split_panes(at, pane, side) {
                    log::warn!("the split is not in the layout: {why}");
                }
            });
        }
    }

    /// The person took pane `key` out of the tab it shares, on the writer thread.
    pub(super) fn whole_later(self: &Arc<Self>, key: &str) {
        if let Some(pane) = self.row_told(key) {
            let desk = Arc::clone(self);
            self.write_later(move || {
                if let Err(why) = desk.pane_out(pane) {
                    log::warn!("the pane is still in its split: {why}");
                }
            });
        }
    }

    /// The person made the tab of pane `key` even, on the writer thread.
    pub(super) fn equalized_later(self: &Arc<Self>, key: &str) {
        let (desk, key) = (Arc::clone(self), key.to_string());
        self.write_later(move || {
            if let Err(why) = desk.equalized(&key) {
                log::warn!("the tab was not made even: {why}");
            }
        });
    }

    /// The person put pane `key` on `side` of pane `beside`, in one go: for tests, as the
    /// window's events do on the writer thread.
    #[cfg(test)]
    pub(super) fn split_told(&self, key: &str, beside: &str, side: Side) {
        if let (Some(pane), Some(at)) = (self.row_told(key), self.row_told(beside)) {
            let _ = self.split_panes(at, pane, side);
        }
    }

    /// The person took pane `key` out of its tab, in one go, as `split_told` tells a split.
    #[cfg(test)]
    pub(super) fn whole_told(&self, key: &str) {
        if let Some(pane) = self.row_told(key) {
            let _ = self.pane_out(pane);
        }
    }

    /// The person asked for a new terminal running `command` on `side` of pane `beside`, in
    /// one go, as `split_told` tells a split: its key.
    #[cfg(test)]
    pub(super) fn split_new_told(
        &self,
        call: &super::Call<'_>,
        beside: &str,
        side: Side,
        command: &'static str,
    ) -> Result<String, String> {
        let group = crate::projects::key(call.project.id);
        let asked = crate::browser::Asked::Agent(command);
        self.person_beside(call, &group, (beside, side), &asked)
    }

    /// The person let go of a divider of pane `key`'s tab (`resized`), on the writer thread.
    pub(super) fn resized_later(
        self: &Arc<Self>,
        key: &str,
        path: &[usize],
        at: usize,
        share: u16,
    ) {
        let (desk, key, path) = (Arc::clone(self), key.to_string(), path.to_vec());
        self.write_later(move || {
            if let Err(why) = desk.resized(&key, &path, at, share) {
                log::warn!("the divider's size was not kept: {why}");
            }
        });
    }

    /// The person put the keys in pane `key` (`focused`), on the writer thread.
    pub(super) fn focused_later(self: &Arc<Self>, key: &str) {
        let (desk, key) = (Arc::clone(self), key.to_string());
        self.write_later(move || {
            if let Err(why) = desk.focused(&key) {
                log::warn!("the pane in focus was not kept: {why}");
            }
        });
    }

    /// The person let go of a divider of the tab pane `key` is in: the branch at `path` gives
    /// its children before `at` `portion` ten-thousandths of it. Only a tree the window drew
    /// whole is resized, every pane of it with a key: a path read in a tree with a pane left
    /// out would name another branch.
    pub(super) fn resized(
        &self,
        key: &str,
        path: &[usize],
        at: usize,
        portion: u16,
    ) -> Result<(), String> {
        if !(1..10_000).contains(&portion) {
            return Err("a divider leaves each side some of its branch".into());
        }
        let Some((pane, keyed)) = self.keyed(key) else {
            return Ok(());
        };
        self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(());
            };
            if !tab.tree.panes().iter().all(|row| keyed.contains(row)) {
                return Ok(());
            }
            let mut tree = tab.tree.clone();
            if !tree.resize(path, at, f64::from(portion) / 10_000.0) || tree == tab.tree {
                return Ok(());
            }
            layout::set_tree(write.tx(), id, &tree, tab.focused)?;
            layout::set_whole(write.tx(), id, false)?;
            told(write, &[Entity::Tab { id }])
        })
    }

    /// The person put the keys in pane `key`, one of a tab of several: it is the tab's pane in
    /// focus, a document the one its stack shows (`show_pane`, as an agent's `show` is).
    pub(super) fn focused(&self, key: &str) -> Result<(), String> {
        let Some((pane, _)) = crate::lock(&self.ledger).get(key).copied() else {
            return Ok(());
        };
        self.show_pane(pane)
    }

    /// The person made the tab pane `key` is in even: each branch's shares by what its
    /// children hold (`Tree::equalize`). As a divider, only a tree every pane of which has a
    /// key: what the window drew is what is made even.
    pub(super) fn equalized(&self, key: &str) -> Result<(), String> {
        let Some((pane, keyed)) = self.keyed(key) else {
            return Ok(());
        };
        self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(());
            };
            let mut tree = tab.tree.clone();
            if !tab.tree.panes().iter().all(|row| keyed.contains(row)) || !tree.equalize() {
                return Ok(());
            }
            layout::set_tree(write.tx(), id, &tree, tab.focused)?;
            layout::set_whole(write.tx(), id, false)?;
            told(write, &[Entity::Tab { id }])
        })
    }

    /// Pane `key`'s row, and the rows of every pane with a key this run.
    fn keyed(&self, key: &str) -> Option<(Row, std::collections::HashSet<Row>)> {
        let ledger = crate::lock(&self.ledger);
        let keyed = ledger.values().map(|(row, _)| *row).collect();
        ledger.get(key).map(|(pane, _)| (*pane, keyed))
    }
}

#[cfg(test)]
#[path = "layout_resize_tests.rs"]
mod tests;
