//! A tab's tree of panes, and what changes it: a pane split in, taken out, a branch resized.
//! Plain data: the store's rows of it are read and written in `nodes`.

use serde::Serialize;

use crate::ledger::Row as PaneId;

/// A tab's panes: one alone, or a branch of trees side by side (`across`) or one above the
/// other, each with its share of the branch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Tree {
    Leaf { pane: PaneId },
    Branch { across: bool, children: Vec<Share> },
}

/// A child of a branch and its share of it (the shares of a branch add up to 1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Share {
    pub share: f64,
    pub tree: Tree,
}

/// The least share a pane keeps of its branch.
pub const LEAST: f64 = 0.1;

impl Tree {
    /// Pane `pane`, alone.
    pub const fn leaf(pane: PaneId) -> Self {
        Self::Leaf { pane }
    }

    /// Its panes, in order: left to right, top to bottom.
    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { pane } => vec![*pane],
            Self::Branch { children, .. } => (children.iter())
                .flat_map(|child| child.tree.panes())
                .collect(),
        }
    }

    /// Whether pane `pane` is one of its.
    pub fn holds(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf { pane: it } => *it == pane,
            Self::Branch { children, .. } => children.iter().any(|child| child.tree.holds(pane)),
        }
    }

    /// Pane `new` beside pane `at`: after it (right of it, or under it) or before. In a branch
    /// of the same way it is one more child, sharing `at`'s share; else `at` becomes a branch of
    /// the two, half each. Whether `at` is one of its.
    pub fn split(&mut self, at: PaneId, new: PaneId, across: bool, after: bool) -> bool {
        match self {
            Self::Leaf { pane } if *pane == at => {
                let (first, second) = if after { (at, new) } else { (new, at) };
                *self = Self::Branch {
                    across,
                    children: vec![
                        Share {
                            share: 0.5,
                            tree: Self::leaf(first),
                        },
                        Share {
                            share: 0.5,
                            tree: Self::leaf(second),
                        },
                    ],
                };
                true
            }
            Self::Leaf { .. } => false,
            Self::Branch {
                across: way,
                children,
            } => {
                let place = (children.iter()).position(|child| child.tree == Self::leaf(at));
                if let (Some(place), true) = (place, *way == across) {
                    let half = children[place].share / 2.0;
                    children[place].share = half;
                    let at_place = if after { place + 1 } else { place };
                    children.insert(
                        at_place,
                        Share {
                            share: half,
                            tree: Self::leaf(new),
                        },
                    );
                    return true;
                }
                (children.iter_mut()).any(|child| child.tree.split(at, new, across, after))
            }
        }
    }

    /// Pane `new` in the place of pane `old`, with its share. Whether `old` is one of its.
    pub fn replace(&mut self, old: PaneId, new: PaneId) -> bool {
        match self {
            Self::Leaf { pane } if *pane == old => {
                *pane = new;
                true
            }
            Self::Leaf { .. } => false,
            Self::Branch { children, .. } => {
                (children.iter_mut()).any(|child| child.tree.replace(old, new))
            }
        }
    }

    /// The tree without pane `pane`: its share goes to the rest of its branch, and a branch left
    /// with one child is that child. None when `pane` was all of it.
    pub fn without(self, pane: PaneId) -> Option<Self> {
        match self {
            Self::Leaf { pane: it } => (it != pane).then_some(self),
            Self::Branch { across, children } => {
                let mut kept: Vec<Share> = (children.into_iter())
                    .filter_map(|child| {
                        let share = child.share;
                        child.tree.without(pane).map(|tree| Share { share, tree })
                    })
                    .collect();
                match kept.len() {
                    0 => None,
                    1 => kept.pop().map(|child| child.tree),
                    _ => {
                        let whole: f64 = kept.iter().map(|child| child.share).sum();
                        for child in &mut kept {
                            child.share = if whole > 0.0 {
                                child.share / whole
                            } else {
                                1.0 / 2.0
                            };
                        }
                        Some(Self::Branch {
                            across,
                            children: kept,
                        })
                    }
                }
            }
        }
    }

    /// The branch at `path` (the place of each child down from the root) gives its first `at`
    /// children `share` of it, the rest the remainder, each side keeping its proportions, and
    /// no child under `LEAST`. Whether there is such a branch.
    pub fn resize(&mut self, path: &[usize], at: usize, share: f64) -> bool {
        if !share.is_finite() {
            return false;
        }
        match (self, path.split_first()) {
            (Self::Branch { children, .. }, Some((&first, rest))) => {
                match children.get_mut(first) {
                    Some(child) => child.tree.resize(rest, at, share),
                    None => false,
                }
            }
            (Self::Branch { children, .. }, None) if at > 0 && at < children.len() => {
                let least = LEAST * at as f64;
                let most = 1.0 - LEAST * (children.len() - at) as f64;
                let share = share.clamp(least, most.max(least));
                let (before, after) = children.split_at_mut(at);
                spread(before, share);
                spread(after, 1.0 - share);
                true
            }
            _ => false,
        }
    }
}

/// `children` given `whole` between them, in their proportions now.
fn spread(children: &mut [Share], whole: f64) {
    let now: f64 = children.iter().map(|child| child.share).sum();
    let count = children.len() as f64;
    for child in children {
        child.share = if now > 0.0 {
            child.share / now * whole
        } else {
            whole / count
        };
    }
}
