//! What a tab's tree holds along each way, and the changes measured by it: every branch's
//! shares made even by the panes its children lay along it, the path down to a pane, a divider
//! beside a pane moved a step, and the same tree named by other ids (the window's keys).

use super::{Share, Tree, joined};
use crate::layout::geometry::Side;

impl<P: Clone + PartialEq> Tree<P> {
    /// How many panes it lays along one way (`across`: side by side, else one above the
    /// other): a pane or a stack is one; a branch of that way the panes of its children
    /// together, a branch of the other way its widest child's.
    pub fn span(&self, across: bool) -> usize {
        match self {
            Self::Leaf { .. } | Self::Stack { .. } => 1,
            Self::Branch {
                across: way,
                children,
            } => {
                let spans = children.iter().map(|child| child.tree.span(across));
                if *way == across {
                    spans.sum()
                } else {
                    spans.max().unwrap_or(1)
                }
            }
        }
    }

    /// Every branch's shares made even by what its children hold: a child's share is the panes
    /// it lays along the branch's way over all the branch lays, so a pane beside a pair takes a
    /// third and the pair two. Whether a share changed.
    pub fn equalize(&mut self) -> bool {
        let Self::Branch { across, children } = self else {
            return false;
        };
        let spans: Vec<f64> = (children.iter())
            .map(|child| f64::from(u32::try_from(child.tree.span(*across)).unwrap_or(u32::MAX)))
            .collect();
        let whole: f64 = spans.iter().sum();
        let mut changed = false;
        for (child, span) in children.iter_mut().zip(spans) {
            let share = span / whole;
            if (child.share - share).abs() > 1e-9 {
                child.share = share;
                changed = true;
            }
            changed |= child.tree.equalize();
        }
        changed
    }

    /// The place of each child down from the root to pane `pane` alone, or to the stack it is
    /// in. None when it is no pane of this tree.
    pub fn path_of(&self, pane: &P) -> Option<Vec<usize>> {
        match self {
            Self::Leaf { .. } | Self::Stack { .. } => self.holds(pane).then(Vec::new),
            Self::Branch { children, .. } => children.iter().enumerate().find_map(|(at, child)| {
                let mut path = child.tree.path_of(pane)?;
                path.insert(0, at);
                Some(path)
            }),
        }
    }

    /// The branch at `path` (the place of each child down from the root): its way and its
    /// children. None where the path leads to a pane or nowhere.
    pub fn branch(&self, path: &[usize]) -> Option<(bool, &[Share<P>])> {
        match (self, path.split_first()) {
            (Self::Branch { children, .. }, Some((first, rest))) => {
                children.get(*first)?.tree.branch(rest)
            }
            (Self::Branch { across, children }, None) => Some((*across, children)),
            _ => None,
        }
    }

    /// Where the divider on `side` of pane `pane` goes, moved `by` of its branch towards
    /// `side`: the branch (`path`), the divider (`at`) and the share the children before it
    /// would have, of the nearest branch of that way that has a divider there (a pane at a
    /// branch's end looks further up). None at the tab's edge on that side. The share is as
    /// `resize` takes it, before it holds each child to `LEAST`.
    pub fn stepped(&self, pane: &P, side: Side, by: f64) -> Option<(Vec<usize>, usize, f64)> {
        let path = self.path_of(pane)?;
        for depth in (0..path.len()).rev() {
            let (here, place) = (&path[..depth], path[depth]);
            let Some((across, children)) = self.branch(here) else {
                continue;
            };
            let at = if side.forward() { place + 1 } else { place };
            if across != side.across() || at == 0 || at >= children.len() {
                continue;
            }
            let before: f64 = children[..at].iter().map(|child| child.share).sum();
            let moved = if side.forward() {
                before + by
            } else {
                before - by
            };
            return Some((here.to_vec(), at, moved));
        }
        None
    }

    /// The divider on `side` of pane `pane` moved `by` of its branch towards `side`
    /// (`stepped`), no child kept under `LEAST`. Whether it moved: a pane at the tab's edge on
    /// that side has none to move.
    pub fn step(&mut self, pane: &P, side: Side, by: f64) -> bool {
        let Some((path, at, share)) = self.stepped(pane, side, by) else {
            return false;
        };
        let was = self.clone();
        self.resize(&path, at, share) && *self != was
    }

    /// The same tree with its panes named by `name`: a pane with no name is left out, its share
    /// to the rest of its branch as `without` gives it; a stack whose pane in sight has none
    /// shows its next named one. None when no pane has a name.
    pub fn named<Q: Clone + PartialEq>(&self, name: &impl Fn(&P) -> Option<Q>) -> Option<Tree<Q>> {
        match self {
            Self::Leaf { pane } => name(pane).map(Tree::leaf),
            Self::Stack { panes, shown } => {
                let at = panes.iter().position(|pane| pane == shown).unwrap_or(0);
                let mut after = (panes.iter().skip(at)).chain(panes.iter().take(at).rev());
                let shown = after.find_map(name)?;
                let panes = panes.iter().filter_map(name).collect();
                Some(Tree::Stack { panes, shown })
            }
            Self::Branch { across, children } => {
                let kept = (children.iter())
                    .filter_map(|child| {
                        let tree = child.tree.named(name)?;
                        Some(Share {
                            share: child.share,
                            tree,
                        })
                    })
                    .collect();
                joined(*across, kept)
            }
        }
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
