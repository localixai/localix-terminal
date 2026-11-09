//! A split the window tells, kept in the layout in the same transaction as its notices: made of
//! two panes, made whole, and its final ratio.

use crate::feed::Entity;
use crate::layout::{self, Tree};
use crate::ledger::Row;
use crate::projects::ProjectId;

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
    /// Each split tab of the project made whole.
    Whole(ProjectId),
}

impl Desk {
    /// The person split the window in project `group` (`halves`, by its panes' keys), or made it
    /// whole there: the layout's tab of the pane in use made of the two, or each split tab of
    /// the project made whole, the pane in focus staying.
    ///
    /// In one go, for tests: the window's events read the split at once (`split_of`) and keep it
    /// on the writer thread (heard.rs).
    #[cfg(test)]
    pub(super) fn split_told(&self, group: &str, halves: Option<crate::browser::Halves<'_>>) {
        if let Some(split) = self.split_of(group, halves) {
            self.split_kept(group, split);
        }
    }

    /// What the split told comes to, read from memory as the window tells it; none when there
    /// is nothing to keep.
    pub(super) fn split_of(
        &self,
        group: &str,
        halves: Option<crate::browser::Halves<'_>>,
    ) -> Option<SplitTold> {
        // The app ending, its panes go: the layout stays as it is for when they open again.
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        match halves {
            Some(halves) => Some(SplitTold::Halves {
                shown: row(halves.shown)?,
                beside: row(halves.beside)?,
                right: halves.right,
            }),
            None => crate::projects::of_key(group).map(SplitTold::Whole),
        }
    }

    /// The split of project `group` kept in the layout.
    pub(super) fn split_kept(&self, group: &str, split: SplitTold) {
        let done = match split {
            SplitTold::Halves {
                shown,
                beside,
                right,
            } => self.split_panes(shown, beside, right),
            SplitTold::Whole(project) => self.unsplit_project(project),
        };
        if let Err(why) = done {
            log::warn!("the split of {group} is not in the layout: {why}");
        }
    }

    /// Every split tab of project `project` in the window in use made whole, its pane in focus
    /// staying.
    fn unsplit_project(&self, project: ProjectId) -> Result<(), String> {
        let now = crate::board::now();
        let wanted = self.in_use();
        self.feed.write(|write| {
            let open = layout::windows(write.tx())?;
            let window = (wanted.filter(|wanted| open.iter().any(|window| window.id == *wanted)))
                .or_else(|| open.first().map(|window| window.id));
            let Some(window) = window else {
                return Ok(());
            };
            let space = (layout::workspaces(write.tx(), window)?.into_iter())
                .find(|space| space.project == project);
            let Some(space) = space else {
                return Ok(());
            };
            for tab in layout::tabs(write.tx(), space.id)? {
                let panes = tab.tree.panes();
                if panes.len() > 1 {
                    let kept = tab.focused.unwrap_or(panes[0]);
                    Self::unsplit_tab(write, tab.id, kept, now)?;
                }
            }
            Ok(())
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
