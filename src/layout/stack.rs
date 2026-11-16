//! A session's documents in its tab's tree: a stack of panes one over the other, one of them in
//! sight, on the right of the session's own pane. A document joins it, is shown in it, and leaves
//! it; the stack ends with its last document.

use super::{Share, Tree};

impl<P: Clone + PartialEq> Tree<P> {
    /// Document `pane` alone: a stack of one.
    pub fn stack_of(pane: P) -> Self {
        Self::Stack {
            panes: vec![pane.clone()],
            shown: pane,
        }
    }

    /// The panes in sight, in order: a stack gives the one it shows.
    pub fn visible(&self) -> Vec<P> {
        match self {
            Self::Leaf { pane } => vec![pane.clone()],
            Self::Stack { shown, .. } => vec![shown.clone()],
            Self::Branch { children, .. } => (children.iter())
                .flat_map(|child| child.tree.visible())
                .collect(),
        }
    }

    /// Its documents, in order: the panes of its stack, none when it has no stack.
    pub fn stacked(&self) -> Vec<P> {
        match self {
            Self::Leaf { .. } => vec![],
            Self::Stack { panes, .. } => panes.clone(),
            Self::Branch { children, .. } => (children.iter())
                .map(|child| child.tree.stacked())
                .find(|panes| !panes.is_empty())
                .unwrap_or_default(),
        }
    }

    /// Document `pane` joins: the last of the tab's one stack and the one in sight. A tree with
    /// no stack yet gets one on the right of all it holds: one more child as wide as each of
    /// panes side by side, else half the room beside them. Whether there was a place; what
    /// panes take a stack is the desk's to say (a session's: terminals, desk/beside.rs).
    pub fn stack(&mut self, pane: P) -> bool {
        let stacked = !self.stacked().is_empty();
        match self {
            Self::Stack { panes, shown } => {
                shown.clone_from(&pane);
                panes.push(pane);
                return true;
            }
            Self::Branch { children, .. } if stacked => {
                return (children.iter_mut())
                    .find(|child| !child.tree.stacked().is_empty())
                    .is_some_and(|child| child.tree.stack(pane));
            }
            Self::Branch {
                across: true,
                children,
            } => {
                let count = children.len() as f64;
                for child in children.iter_mut() {
                    child.share *= count / (count + 1.0);
                }
                let share = 1.0 / (count + 1.0);
                children.push(Share {
                    share,
                    tree: Self::stack_of(pane),
                });
                return true;
            }
            Self::Leaf { .. } | Self::Branch { .. } => {}
        }
        let share = |tree: Self| Share { share: 0.5, tree };
        *self = Self::Branch {
            across: true,
            children: vec![share(self.clone()), share(Self::stack_of(pane))],
        };
        true
    }

    /// The document in sight: the one its stack shows, none when it has no stack.
    pub fn doc(&self) -> Option<P> {
        match self {
            Self::Leaf { .. } => None,
            Self::Stack { shown, .. } => Some(shown.clone()),
            Self::Branch { children, .. } => children.iter().find_map(|child| child.tree.doc()),
        }
    }

    /// Pane `left` is in the left half of a tab of two halves side by side: they change sides
    /// when it was in the right one. Whether they did.
    pub fn order(&mut self, left: &P) -> bool {
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

    /// Document `pane` goes before document `before` in its stack (last with none); the one in
    /// sight stays in sight. Whether the order changed: not when either is no document of it.
    pub fn place(&mut self, pane: &P, before: Option<&P>) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Stack { panes, .. } => {
                let mut placed: Vec<P> = panes.iter().filter(|it| *it != pane).cloned().collect();
                let at = match before {
                    Some(before) => placed.iter().position(|it| it == before),
                    None => Some(placed.len()),
                };
                let Some(at) = at.filter(|_| placed.len() < panes.len()) else {
                    return false;
                };
                placed.insert(at, pane.clone());
                let changed = placed != *panes;
                *panes = placed;
                changed
            }
            Self::Branch { children, .. } => {
                (children.iter_mut()).any(|child| child.tree.place(pane, before))
            }
        }
    }

    /// Document `pane` is the one its stack shows. Whether it is a document of it.
    pub fn show(&mut self, pane: &P) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Stack { panes, shown } => {
                let found = panes.contains(pane);
                if found {
                    shown.clone_from(pane);
                }
                found
            }
            Self::Branch { children, .. } => children.iter_mut().any(|child| child.tree.show(pane)),
        }
    }
}

/// The stack of `panes` showing `shown`, without `pane`: the one after it in sight when it
/// was, else the one before. None when it was the last.
pub(super) fn without<P: Clone + PartialEq>(
    mut panes: Vec<P>,
    shown: P,
    pane: &P,
) -> Option<Tree<P>> {
    let Some(at) = panes.iter().position(|it| it == pane) else {
        return Some(Tree::Stack { panes, shown });
    };
    panes.remove(at);
    let next = panes.get(at).or_else(|| panes.last()).cloned()?;
    let shown = if shown == *pane { next } else { shown };
    Some(Tree::Stack { panes, shown })
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
