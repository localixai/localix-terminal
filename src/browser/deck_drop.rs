//! A tab carried over the pages and a divider dragged: the zones of the panes a carried tab
//! is aimed by (`carry_zones.rs`, the panes as the layout keeps them), where it lands (beside
//! the pane it is aimed at, on that side), and what that makes of the panes, which the window
//! is laid out by while the tab stands there (`deck_preview.rs`); and a divider's share as the
//! person drags it or makes its branch even.

use super::super::carry_zones::Zones;
use super::super::split::{self, Held, Own};
use super::super::strip::Key;
use super::super::{Drawn, Side};
use super::Deck;
use crate::layout::Tree;
use crate::layout::geometry::{self, Laid, Rect};

/// Where a carried tab lands: the tree it makes of the tab shown, and the split the layout is
/// told (the pane it goes beside, and on which side of it).
#[derive(Clone)]
pub(in super::super) struct Landed {
    pub tree: Drawn,
    pub beside: Key,
    pub side: Side,
}

/// A landing and the panes it lays out (the carried tab's among them), as the window will lay
/// them once the layout hears of it.
pub(in super::super) struct Plan {
    pub landed: Landed,
    pub laid: Laid<Key>,
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

    /// The panes' part of the window, as one rectangle.
    pub(in super::super) fn pages_rect(&self) -> Rect {
        let (size, left, top) = self.area(false);
        Rect::new(left, top, size.width, size.height)
    }

    /// Whether `x`, `y` of the window is on the pages, not past their edges.
    pub(in super::super) fn on_pages(&self, x: f64, y: f64) -> bool {
        let area = self.pages_rect();
        x >= area.x && y >= area.y && x <= area.x + area.width && y <= area.y + area.height
    }

    /// The panes as the layout keeps them, each where it lies (not where a carried tab's
    /// landing puts it meanwhile): a split tab's, or the one shown alone over the whole part.
    pub(in super::super) fn homes(&self) -> Vec<(Key, Rect)> {
        match self.panes_kept() {
            Some(panes) => panes.panes,
            None => self
                .shown_key()
                .map(|shown| (shown, self.pages_rect()))
                .into_iter()
                .collect(),
        }
    }

    /// The zones a tab carried over the panes is aimed by, taken from them as they lie.
    pub(in super::super) fn zones(&self) -> Zones<Key, Plan> {
        Zones::new(self.homes(), self.pages_rect())
    }

    /// What landing tab `key` on `side` of pane `beside` makes of the panes, as the person had
    /// made the tab shown; none where nothing changes.
    pub(in super::super) fn plan(&self, key: Key, beside: Key, side: Side) -> Option<Plan> {
        let landed = self.landed(key, (beside, side), self.split.own.as_ref())?;
        let tree = landed.tree.named(&|pane: &String| Key::parse(pane))?;
        let laid = geometry::lay(&tree, self.pages_rect(), split::SPACING);
        Some(Plan { landed, laid })
    }

    /// Where tab `key` lands on `side` of pane `beside`, from the tab shown as the person had
    /// made it (`was`): beside that pane, on that side, as the layout puts it; carried over the
    /// other of two panes, the two change sides. None where nothing changes: over itself, over
    /// a pane the layout does not take it beside, or within a tab of more than two.
    pub(in super::super) fn landed(
        &self,
        key: Key,
        (beside, side): (Key, Side),
        was: Option<&Own>,
    ) -> Option<Landed> {
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

    /// Whether a divider is held now: taken hold of and not yet let go.
    pub(super) fn dragging(&self) -> bool {
        (self.split.held.iter()).any(|held| held.grabbed.is_some())
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
