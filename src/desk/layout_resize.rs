//! Persist a split's final ratio, off the UI thread and in the same transaction as its notice.

use crate::feed::Entity;
use crate::layout::{self, Tree};

use super::Desk;
use super::layout::told;

impl Desk {
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
