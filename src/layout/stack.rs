//! A session's documents in its tab's tree: a stack of panes one over the other, one of them in
//! sight, on the right of the session's own pane. A document joins it, is shown in it, and leaves
//! it; the stack ends with its last document.

use super::{Share, Tree};
use crate::ledger::Row as PaneId;

impl Tree {
    /// Document `pane` alone: a stack of one.
    pub fn stack_of(pane: PaneId) -> Self {
        Self::Stack {
            panes: vec![pane],
            shown: pane,
        }
    }

    /// The panes in sight, in order: a stack gives the one it shows.
    pub fn visible(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { pane } => vec![*pane],
            Self::Stack { shown, .. } => vec![*shown],
            Self::Branch { children, .. } => (children.iter())
                .flat_map(|child| child.tree.visible())
                .collect(),
        }
    }

    /// Its documents, in order: the panes of its stack, none when it has no stack.
    pub fn stacked(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { .. } => vec![],
            Self::Stack { panes, .. } => panes.clone(),
            Self::Branch { children, .. } => (children.iter())
                .map(|child| child.tree.stacked())
                .find(|panes| !panes.is_empty())
                .unwrap_or_default(),
        }
    }

    /// Document `pane` joins: the last of its stack and the one in sight; a pane alone gets a
    /// stack on its right, half the room each. Whether there was a place: panes side by side
    /// with no stack have none.
    pub fn stack(&mut self, pane: PaneId) -> bool {
        match self {
            Self::Leaf { .. } => {
                let share = |tree: Self| Share { share: 0.5, tree };
                *self = Self::Branch {
                    across: true,
                    children: vec![share(self.clone()), share(Self::stack_of(pane))],
                };
                true
            }
            Self::Stack { panes, shown } => {
                panes.push(pane);
                *shown = pane;
                true
            }
            Self::Branch { children, .. } => (children.iter_mut())
                .find(|child| !child.tree.stacked().is_empty())
                .is_some_and(|child| child.tree.stack(pane)),
        }
    }

    /// The document in sight: the one its stack shows, none when it has no stack.
    pub fn doc(&self) -> Option<PaneId> {
        match self {
            Self::Leaf { .. } => None,
            Self::Stack { shown, .. } => Some(*shown),
            Self::Branch { children, .. } => children.iter().find_map(|child| child.tree.doc()),
        }
    }

    /// Pane `left` is in the left half of a tab of two halves side by side: they change sides
    /// when it was in the right one. Whether they did.
    pub fn order(&mut self, left: PaneId) -> bool {
        let Self::Branch {
            across: true,
            children,
        } = self
        else {
            return false;
        };
        let swap = children.len() == 2 && children[1].tree.holds(left);
        if swap {
            children.swap(0, 1);
        }
        swap
    }

    /// Document `pane` is the one its stack shows. Whether it is a document of it.
    pub fn show(&mut self, pane: PaneId) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Stack { panes, shown } => {
                let found = panes.contains(&pane);
                if found {
                    *shown = pane;
                }
                found
            }
            Self::Branch { children, .. } => children.iter_mut().any(|child| child.tree.show(pane)),
        }
    }
}

/// The stack of `panes` showing `shown`, without `pane`: the one after it in sight when it
/// was, else the one before. None when it was the last.
pub(super) fn without(mut panes: Vec<PaneId>, shown: PaneId, pane: PaneId) -> Option<Tree> {
    let Some(at) = panes.iter().position(|it| *it == pane) else {
        return Some(Tree::Stack { panes, shown });
    };
    panes.remove(at);
    let next = panes.get(at).or_else(|| panes.last()).copied()?;
    let shown = if shown == pane { next } else { shown };
    Some(Tree::Stack { panes, shown })
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
