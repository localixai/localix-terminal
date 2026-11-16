//! A tab's tree in the window (docs/windows.md): the window draws the tree of the tab its shown
//! pane is in, as the layout last told it, laid out by the layout's geometry. What the person
//! did to it that the layout has not told back is all the window keeps: a split made or undone
//! (`Own`), a divider held or let go of (`Held`), and the pane they put the keys in (`focus`).
//! Each waits for the layout and goes once it has heard; none is a tree of the window's own.

use super::{Arranged, Arranging, Drawn};
use crate::layout::geometry::Spacing;

/// The space between two panes, in points: the window's glass, as a line.
pub(super) const GAP: f64 = 1.0;
/// The least a pane keeps, in points, however a divider is dragged or the window sized.
pub(super) const LEAST_POINTS: f64 = 190.0;
/// How wide a divider is to take hold of, in points: the drawn point and four either side.
pub(super) const LINE: f64 = 9.0;
/// How the window lays a tab's tree out.
pub(super) const SPACING: Spacing = Spacing {
    gap: GAP,
    least: LEAST_POINTS,
};

/// What the person made of a tab before the layout heard of it: the tree they made (a pane
/// alone: the tab made whole), and the panes the layout drew of it then (`base`), so a tell
/// still as old as that keeps it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Own {
    pub tree: Drawn,
    pub base: Vec<String>,
}

/// A divider the person holds or let go of: its tab by the panes it draws, its branch (`path`),
/// the child it comes before, the share of the children before it, and the share when they
/// took hold of it (a drag moves from there; none once let go, waiting for the layout).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Held {
    pub panes: Vec<String>,
    pub path: Vec<usize>,
    pub at: usize,
    pub share: f64,
    pub grabbed: Option<f64>,
}

/// What of the shown tab's tree waits for the layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Split {
    pub own: Option<Own>,
    /// Each divider held or let go of and not yet told back, the latest last.
    pub held: Vec<Held>,
    /// The pane whose focus the window told the layout, until it tells it back.
    pub focus: Option<String>,
}

/// The panes tab `tab` draws: its pane in focus alone while it is kept whole, else those in
/// sight.
pub(super) fn drawn(tab: &Arranging) -> Vec<String> {
    let sight = tab.tree.visible();
    if !tab.whole {
        return sight;
    }
    let focused = tab
        .focused
        .clone()
        .filter(|focused| sight.contains(focused));
    focused
        .or_else(|| sight.first().cloned())
        .into_iter()
        .collect()
}

/// The share of its branch the children of the branch at `path` before `at` have.
pub(super) fn share_at(tree: &Drawn, path: &[usize], at: usize) -> Option<f64> {
    let (_, children) = tree.branch(path)?;
    let whole: f64 = children.iter().map(|child| child.share).sum();
    let before: f64 = children.get(..at)?.iter().map(|child| child.share).sum();
    (whole > 0.0).then(|| before / whole)
}

impl Split {
    /// The tree the window draws for pane `shown`, in tab `tab` as the layout keeps it (none:
    /// in none yet). What the person made of the tab, while the layout still draws it as it
    /// did then: their tree when it holds `shown`, else `shown` alone. Else the layout's, a
    /// stack showing `shown` when it is one of its documents, and a divider held where the
    /// person holds it. None for a pane drawn alone.
    pub(super) fn tree(&self, tab: Option<&Arranging>, shown: &str) -> Option<Drawn> {
        let base = tab.map_or_else(|| vec![shown.to_string()], drawn);
        let own = self.own.as_ref().filter(|own| own.base == base);
        let mut tree = match own {
            Some(own) if own.tree.holds(&shown.to_string()) => own.tree.clone(),
            Some(_) => return None,
            None => tab.filter(|tab| !tab.whole)?.tree.clone(),
        };
        tree.show(&shown.to_string());
        let sight = tree.visible();
        if sight.len() < 2 {
            return None;
        }
        for held in self.held.iter().filter(|held| held.panes == sight) {
            tree.resize(&held.path, held.at, held.share);
        }
        Some(tree)
    }

    /// The layout told tab `tab` (none: it keeps pane `shown` in none) again: what the person
    /// made of it goes once the layout draws it so, or draws it otherwise than it did when
    /// they made it (it heard and kept another, or another change came first); a divider let
    /// go of, once the layout keeps its share or the tab is another; the focus told, once the
    /// layout keeps it.
    pub(super) fn follow(&mut self, tab: Option<&Arranging>, shown: &str) {
        let now = tab.map_or_else(|| vec![shown.to_string()], drawn);
        if let Some(own) = &self.own
            && (own.tree.visible() == now || own.base != now)
        {
            self.own = None;
        }
        self.held.retain(|held| {
            let saved = tab
                .filter(|tab| tab.tree.visible() == held.panes)
                .map(|tab| share_at(&tab.tree, &held.path, held.at));
            match saved {
                _ if held.grabbed.is_some() => true,
                Some(Some(saved)) => (saved - held.share).abs() > 0.00011,
                _ => false,
            }
        });
        if tab.is_some_and(|tab| tab.focused == self.focus) {
            self.focus = None;
        }
    }
}

/// The tab to put beside the shown one when the person splits the window: the one after it
/// among the project's tabs (`tabs`, in their order), else the one before.
pub(super) fn partner<K: Copy + PartialEq>(tabs: &[K], shown: K) -> Option<K> {
    let at = tabs.iter().position(|tab| *tab == shown)?;
    tabs.get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|before| tabs.get(before)))
        .copied()
}

/// The pane after `shown` among `sight` (`forward`), or before it, round from the last to the
/// first. None with `shown` alone or not among them.
pub(super) fn round<K: Clone + PartialEq>(sight: &[K], shown: &K, forward: bool) -> Option<K> {
    let at = sight.iter().position(|key| key == shown)?;
    let count = sight.len();
    let next = if forward { at + 1 } else { at + count - 1 };
    sight.get(next % count).filter(|key| *key != shown).cloned()
}

/// The pane that takes the keys once pane `key` of `tree` closes, the panes in `sight`: the one
/// used last of the rest (`recent`, the latest first), else its neighbour in its branch (the
/// one after it, else the one before), which takes its share: that child's nearest pane.
pub(super) fn taker(
    tree: &Drawn,
    recent: &[String],
    key: &str,
    sight: &[String],
) -> Option<String> {
    let used = recent
        .iter()
        .find(|used| *used != key && sight.contains(used));
    if let Some(used) = used {
        return Some(used.clone());
    }
    let path = tree.path_of(&key.to_string())?;
    let (place, branch) = path.split_last()?;
    let (_, children) = tree.branch(branch)?;
    let (child, after) = match children.get(place + 1) {
        Some(next) => (next, true),
        None => (children.get(place.checked_sub(1)?)?, false),
    };
    let visible = child.tree.visible();
    let pane = if after {
        visible.first()
    } else {
        visible.last()
    };
    pane.cloned()
}

/// The panes of project `arranged`'s split that the side lists as one while another tab is
/// shown: the tab it shows when that one is split, else its first split tab. A session and its
/// documents are no such split (session.rs): the side lists the session alone. None while the
/// person made that tab whole and the layout has not heard (`own`).
pub(super) fn listed(arranged: &Arranged, own: Option<&Own>) -> Option<Vec<String>> {
    let split = |tab: &&Arranging| drawn(tab).len() >= 2 && tab.docs.is_empty();
    let shown = (arranged.tabs.iter())
        .filter(split)
        .find(|tab| (arranged.shown.as_ref()).is_some_and(|shown| tab.tree.holds(shown)));
    let panes = drawn(shown.or_else(|| arranged.tabs.iter().find(split))?);
    let undone = own.is_some_and(|own| own.base == panes && own.tree.visible().len() < 2);
    (!undone).then_some(panes)
}

/// What is laid out while a list is open over the pages (`over`: a menu, a dialog, an
/// address's suggestions): where the strip lies over the pages (`raised`) the window stays as
/// it is, split; where it cannot, the pages stand aside for the list, the split with them.
pub(super) fn under<T>(laid: Option<T>, over: bool, raised: bool) -> Option<T> {
    laid.filter(|_| raised || !over)
}

/// The least and most share the children before `at` of a branch of `children` take of `room`
/// points along its way, where each child keeps the least its panes need (`SPACING`).
pub(super) fn bounds(
    children: &[crate::layout::Share<String>],
    across: bool,
    at: usize,
    room: f64,
) -> (f64, f64) {
    let least = |child: &crate::layout::Share<String>| {
        let span = child.tree.span(across) as f64;
        let need = span * LEAST_POINTS + (span - 1.0) * GAP;
        (need / room.max(1.0)).max(crate::layout::LEAST)
    };
    let (before, after) = children.split_at(at.min(children.len()));
    let low: f64 = before.iter().map(least).sum();
    let high = 1.0 - after.iter().map(least).sum::<f64>();
    if low <= high {
        (low, high)
    } else {
        let even = low / (low + (1.0 - high));
        (even, even)
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
