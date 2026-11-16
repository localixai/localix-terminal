//! A tab carried over the pages and a divider dragged: where a carried tab lands (beside the
//! pane it is over, on the side of the edge of that pane it is nearest), the window laid out so
//! as it is carried, and a divider's share as the person drags it or makes its branch even.

use super::super::split::{self, Held, Own};
use super::super::strip::Key;
use super::super::{Drawn, Side};
use super::{Deck, Landing, Over};
use crate::layout::Tree;
use crate::layout::geometry::Rect;

/// A whole window's middle is not a split target.
const DEAD_MIDDLE: f64 = 0.18;

/// Where a carried tab lands: the tree it makes of the tab shown, and the split the layout is
/// told (the pane it goes beside, and on which side of it).
pub(in super::super) struct Landed {
    pub tree: Drawn,
    pub beside: Key,
    pub side: Side,
}

/// The side of `rect` whose edge point `x`, `y` is nearest, by the part of the rectangle it is
/// from it, and that part (0 on the edge, a half in the middle).
fn nearest(rect: Rect, x: f64, y: f64) -> (Side, f64) {
    let across = (x - rect.x) / rect.width.max(1.0);
    let down = (y - rect.y) / rect.height.max(1.0);
    let edges = [
        (Side::Left, across),
        (Side::Right, 1.0 - across),
        (Side::Up, down),
        (Side::Down, 1.0 - down),
    ];
    (edges.into_iter())
        .min_by(|one, other| one.1.total_cmp(&other.1))
        .unwrap_or((Side::Right, 0.5))
}

impl Deck {
    /// The panes the layout draws of the shown pane's tab now: what the person's own change of
    /// it waits to see heard.
    pub(in super::super) fn base(&self) -> Vec<String> {
        let Some(shown) = self.shown_key().map(Key::name) else {
            return vec![];
        };
        self.tab_holding(&shown)
            .map_or_else(|| vec![shown], |tab| split::drawn(&tab))
    }

    /// Where `x`, `y` of the window is for a tab carried there: a split window's pane and which
    /// half of it, a whole one's even halves (its middle none), or off the pages.
    pub(in super::super) fn over(&self, x: f64, y: f64) -> Over {
        let (size, left, top) = self.area(false);
        // Off the pages, past the window's edges too, it goes nowhere.
        if x < left || y < top || x > left + size.width || y > top + size.height {
            return Over::Off;
        }
        if let Some(panes) = self.panes() {
            let pane = panes.panes.iter().find(|(_, rect)| rect.contains(x, y));
            return pane.map_or(Over::Middle, |(key, rect)| {
                Over::Pane(*key, nearest(*rect, x, y).0)
            });
        }
        let Some(shown) = self.shown_key() else {
            return Over::Off;
        };
        // A whole window splits only for a tab taken well to one edge: its middle is no side.
        let (side, from) = nearest(Rect::new(left, top, size.width, size.height), x, y);
        if 0.5 - from < DEAD_MIDDLE {
            Over::Middle
        } else {
            Over::Pane(shown, side)
        }
    }

    /// Where tab `key` lands let go of `over`, from the tab shown as the person had made it
    /// (`was`): beside the pane it is over, on that side, as the layout puts it; carried over
    /// the other of two panes, the two change sides. None where nothing changes: over itself,
    /// over a pane the layout does not take it beside, or within a tab of more than two.
    pub(in super::super) fn landed(
        &self,
        key: Key,
        over: Over,
        was: Option<&Own>,
    ) -> Option<Landed> {
        let Over::Pane(beside, side) = over else {
            return None;
        };
        let shown = self.shown_key()?.name();
        let (name, at) = (key.name(), beside.name());
        if beside == key {
            return None;
        }
        let before = split::Split {
            own: was.cloned(),
            ..split::Split::default()
        };
        let tab = self.tab_holding(&shown);
        let mut tree = before
            .tree(tab.as_ref(), &shown)
            .unwrap_or_else(|| Tree::leaf(shown));
        if tree.holds(&name) {
            let Tree::Branch { children, .. } = &mut tree else {
                return None;
            };
            if children.len() != 2 || tree_panes(children) != 2 {
                return None;
            }
            children.reverse();
            let across = matches!(tree, Tree::Branch { across: true, .. });
            let side = match (across, tree.visible().first() == Some(&at)) {
                (true, true) => Side::Right,
                (true, false) => Side::Left,
                (false, true) => Side::Down,
                (false, false) => Side::Up,
            };
            return Some(Landed { tree, beside, side });
        }
        if !self.takes(beside, key) || !tree.split(&at, name, side.across(), side.forward()) {
            return None;
        }
        Some(Landed { tree, beside, side })
    }

    /// Tab `key` carried over the pages at `at` (a point of the window), or off them (none):
    /// the window is laid out as it will be where the tab lands, and back as it was off the
    /// pages. Over a whole window's middle it stays as it is.
    pub(in super::super) fn drop_over(&mut self, key: Key, at: Option<(f64, f64)>) {
        let over = at.map_or(Over::Off, |(x, y)| self.over(x, y));
        let side = match over {
            Over::Middle => return,
            Over::Off => None,
            Over::Pane(pane, side) => Some((pane, side)),
        };
        if side == self.drop {
            return;
        }
        self.drop = side;
        // Where it lands is worked out from the window as it was before the tab came over it.
        let was = (self.landing.as_ref())
            .map_or_else(|| self.split.own.clone(), |landing| landing.was.clone());
        match self.landed(key, over, was.as_ref()) {
            Some(landed) => {
                let base = self.base();
                self.landing.get_or_insert(Landing { was });
                self.split.own = Some(Own {
                    tree: landed.tree,
                    base,
                });
            }
            None => {
                if let Some(landing) = self.landing.take() {
                    self.split.own = landing.was;
                }
            }
        }
        self.layout();
    }

    /// The tab carried over the pages taken off them: the window as it was before.
    pub(in super::super) fn drop_off(&mut self) {
        let was = self.landing.take();
        self.drop = None;
        if let Some(landing) = was {
            self.split.own = landing.was;
            self.layout();
        }
    }

    /// The tab carried over the pages let go of: what the person had made of the tab shown
    /// before it came over them, to work out where it landed from.
    pub(in super::super) fn land(&mut self) -> Option<Own> {
        self.drop = None;
        match self.landing.take() {
            Some(landing) => landing.was,
            None => self.split.own.clone(),
        }
    }

    /// Divider `at` of the tree shown dragged `moved` points since the person took hold of it;
    /// let go of when `done`. Its share is held where every pane keeps its least, and kept in
    /// the window until the layout tells it back. What the layout is told once let go: the
    /// branch, the divider and its share.
    pub(in super::super) fn drag_split(
        &mut self,
        at: usize,
        moved: i32,
        done: bool,
    ) -> Option<Held> {
        let held = self.holding(
            at,
            |held, _| held.grabbed.unwrap_or(held.share),
            f64::from(moved),
        )?;
        self.finish_hold(held, done)
    }

    /// Divider `at`'s branch made even by what its children hold (`Tree::equalize`'s share for
    /// that divider), as a drag let go of.
    pub(in super::super) fn split_even(&mut self, at: usize) -> Option<Held> {
        let even = |held: &Held, tree: &Drawn| {
            let (across, children) = tree.branch(&held.path)?;
            let span = |child: &crate::layout::Share<String>| child.tree.span(across) as f64;
            let all: f64 = children.iter().map(span).sum();
            Some(children.get(..held.at)?.iter().map(span).sum::<f64>() / all)
        };
        let held = self.holding(at, |held, tree| even(held, tree).unwrap_or(held.share), 0.0)?;
        self.finish_hold(held, true)
    }

    /// Divider `at` held: its share from `from` (of the hold and the tree), moved `moved`
    /// points along its branch, held to what keeps each pane its least.
    pub(super) fn holding(
        &self,
        at: usize,
        from: impl Fn(&Held, &Drawn) -> f64,
        moved: f64,
    ) -> Option<Held> {
        let panes = self.panes()?;
        let divider = panes.dividers.get(at)?.clone();
        let tree = self.tree()?;
        // A tree with a pane this window does not hold draws other branches than the layout
        // keeps: it is not resized from here.
        if Some(tree.visible().len()) != self.drawn_whole().map(|tree| tree.visible().len()) {
            return None;
        }
        let sight = tree.visible();
        let same = |held: &&Held| {
            held.panes == sight && held.path == divider.path && held.at == divider.at
        };
        let mut held = (self.split.held.iter().find(same).cloned()).unwrap_or(Held {
            panes: sight,
            path: divider.path.clone(),
            at: divider.at,
            share: split::share_at(&tree, &divider.path, divider.at)?,
            grabbed: None,
        });
        let start = from(&held, &tree);
        let (across, children) = tree.branch(&divider.path)?;
        let (low, high) = split::bounds(children, across, divider.at, divider.room);
        held.share = (start + moved / divider.room.max(1.0)).clamp(low, high);
        held.grabbed = Some(start);
        Some(held)
    }

    /// The hold kept, let go of when `done`, and the window laid out with it: the hold to tell
    /// the layout once let go.
    pub(super) fn finish_hold(&mut self, mut held: Held, done: bool) -> Option<Held> {
        if done {
            held.grabbed = None;
        }
        let same = |other: &Held| {
            other.panes == held.panes && other.path == held.path && other.at == held.at
        };
        self.split.held.retain(|other| !same(other));
        self.split.held.push(held.clone());
        self.layout();
        done.then_some(held)
    }

    /// The tree the window draws, before panes it does not hold are left out.
    fn drawn_whole(&self) -> Option<Drawn> {
        let shown = self.shown_key()?.name();
        self.split.tree(self.tab_holding(&shown).as_ref(), &shown)
    }
}

/// How many panes in sight `children` hold.
fn tree_panes(children: &[crate::layout::Share<String>]) -> usize {
    children
        .iter()
        .map(|child| child.tree.visible().len())
        .sum()
}
