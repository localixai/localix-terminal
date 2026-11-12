//! A split the window tells, kept in the layout in the same transaction as its notices: made of
//! two panes, made whole, and its final ratio.

use crate::feed::Entity;
use crate::layout::{self, Tree};
use crate::ledger::Row;

use super::Desk;
use super::layout::told;

/// A split the window told, by the rows of its panes: what the layout is to make of it.
#[derive(Debug, Clone, Copy)]
pub(super) enum SplitTold {
    /// The tab of pane `shown` made of it and `beside`, on its right or its left.
    Halves {
        shown: Row,
        beside: Row,
        right: bool,
    },
    /// The tab these two panes shared made whole (one of them may be gone by now).
    Whole([Option<Row>; 2]),
}

impl Desk {
    /// The person split the window (`halves`, by its panes' keys): the layout's tab of the pane
    /// in use made of the two.
    ///
    /// In one go, for tests: the window's events read the split at once (`split_of`) and keep it
    /// on the writer thread (heard.rs).
    #[cfg(test)]
    pub(super) fn split_told(&self, halves: crate::browser::Halves<'_>) {
        if let Some(split) = self.split_of(halves, false) {
            self.split_kept(split);
        }
    }

    /// The person made the split of `halves` whole, as `split_told` tells a split.
    #[cfg(test)]
    pub(super) fn whole_told(&self, halves: crate::browser::Halves<'_>) {
        if let Some(split) = self.split_of(halves, true) {
            self.split_kept(split);
        }
    }

    /// What the split told comes to (`whole`: it is no more), read from memory as the window
    /// tells it; none when there is nothing to keep.
    pub(super) fn split_of(
        &self,
        halves: crate::browser::Halves<'_>,
        whole: bool,
    ) -> Option<SplitTold> {
        // The app ending, its panes go: the layout stays as it is for when they open again.
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        if whole {
            let rows = [row(halves.shown), row(halves.beside)];
            return rows
                .iter()
                .any(Option::is_some)
                .then_some(SplitTold::Whole(rows));
        }
        Some(SplitTold::Halves {
            shown: row(halves.shown)?,
            beside: row(halves.beside)?,
            right: halves.right,
        })
    }

    /// The split told kept in the layout.
    pub(super) fn split_kept(&self, split: SplitTold) {
        let done = match split {
            SplitTold::Halves {
                shown,
                beside,
                right,
            } => self.split_panes(shown, beside, right),
            SplitTold::Whole(rows) => self.unsplit_of(rows).map(drop),
        };
        if let Err(why) = done {
            log::warn!("the split is not in the layout: {why}");
        }
    }

    /// The tab the panes `rows` shared made whole, its pane in focus staying and the other a
    /// tab right after: the panes it had. Each other split tab is left as it is.
    fn unsplit_of(&self, rows: [Option<Row>; 2]) -> Result<Vec<Row>, String> {
        let now = crate::board::now();
        self.feed.write(|write| {
            // A session keeps its documents: it shows one pane alone, and stays one tab.
            if Self::whole_session(write, rows)? {
                return Ok(vec![]);
            }
            for row in rows.into_iter().flatten() {
                let Some(id) = layout::tab_of(write.tx(), row)? else {
                    continue;
                };
                let Some(tab) = layout::tab(write.tx(), id)? else {
                    continue;
                };
                let panes = tab.tree.panes();
                // Only the tab the two shared: one of them moved to another split is not it.
                let shared = rows.iter().flatten().all(|row| panes.contains(row));
                if panes.len() > 1 && shared {
                    let kept = tab.focused.unwrap_or(panes[0]);
                    Self::unsplit_tab(write, id, kept, now)?;
                    return Ok(panes);
                }
            }
            Ok(vec![])
        })
    }

    pub(super) fn split_resized(&self, key: &str, portion: u16) -> Result<(), String> {
        if !(1000..=9000).contains(&portion) {
            return Err("each half of a split keeps at least a tenth".into());
        }
        let Some((pane, _)) = crate::lock(&self.ledger).get(key).copied() else {
            return Ok(());
        };
        self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(());
            };
            let Tree::Branch {
                across: true,
                children,
            } = &tab.tree
            else {
                return Ok(());
            };
            if children.len() != 2 {
                return Ok(());
            }
            let mut tree = tab.tree.clone();
            if !tree.resize(&[], 1, f64::from(portion) / 10_000.0) || tree == tab.tree {
                return Ok(());
            }
            layout::set_tree(write.tx(), id, &tree, tab.focused)?;
            told(write, &[Entity::Tab { id }])
        })
    }
}

#[cfg(test)]
#[path = "layout_resize_tests.rs"]
mod tests;
