//! Split view: two tabs side by side, never more. The window keeps only the tab beside the one
//! shown and which side it is on: the shown tab is always the half in use, so a click in the
//! other half makes it the one in use (the two change places as roles, not as sides). Another
//! tab picked, the window shows it whole, and its project keeps the split: one of its two tabs
//! picked again, the window splits as it was. Made whole by the person, or a half's tab closed,
//! the split is gone.

use super::strip::Key;

/// The tab beside the one shown, and whether it is on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Beside {
    pub key: Key,
    pub right: bool,
}

/// The two halves' tabs, left and right, when the window is split: `beside` is still there
/// (`there`) and is not the one shown.
pub(super) fn pair(
    shown: Option<Key>,
    beside: Option<Beside>,
    there: impl Fn(Key) -> bool,
) -> Option<(Key, Key)> {
    let (shown, beside) = (shown?, beside?);
    if beside.key == shown || !there(beside.key) {
        return None;
    }
    Some(if beside.right {
        (shown, beside.key)
    } else {
        (beside.key, shown)
    })
}

/// What is beside once `now` is shown instead of `was`: the tab beside picked, they change
/// roles in place; any other, the window is whole (its project keeps the split).
pub(super) fn after_show(beside: Option<Beside>, was: Option<Key>, now: Key) -> Option<Beside> {
    let beside = beside?;
    if beside.key != now {
        return None;
    }
    was.filter(|was| *was != now).map(|was| Beside {
        key: was,
        right: !beside.right,
    })
}

/// The tab to put beside the shown one when the person splits the window: the one after it
/// among the project's tabs (`tabs`, in their order), else the one before.
pub(super) fn partner(tabs: &[Key], shown: Key) -> Option<Key> {
    let at = tabs.iter().position(|tab| *tab == shown)?;
    tabs.get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|before| tabs.get(before)))
        .copied()
}

/// Where tab `key`, carried over the pages, lands when let go of over the right half (`right`)
/// or the left: the tab beside the shown one then. The window lays it out so as the tab is
/// carried, the halves as they will be. A whole window gets it beside; a split one, in the
/// half not in use (the shown tab carried there, the two change sides). None when the half is
/// the one in use, or the tab is the shown one over a whole window: nothing to lay out before
/// it is let go of.
pub(super) fn landed(
    beside: Option<Beside>,
    shown: Option<Key>,
    pair: Option<(Key, Key)>,
    key: Key,
    right: bool,
) -> Option<Beside> {
    let Some((left_half, right_half)) = pair else {
        return (Some(key) != shown).then_some(Beside { key, right });
    };
    let into = if right { right_half } else { left_half };
    if Some(into) == shown {
        return None;
    }
    if Some(key) == shown {
        return beside.map(|beside| Beside {
            key: beside.key,
            right: !beside.right,
        });
    }
    Some(Beside { key, right })
}

/// A project's split as kept while the window shows another: the tab in use, the one beside,
/// by their keys (`w1`, `t2`), and whether that one is on the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Kept {
    pub shown: String,
    pub beside: String,
    pub right: bool,
}

/// What the window told the layout of its project's split and is waiting to hear back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Told {
    /// Nothing waits: the window follows the layout.
    Nothing,
    /// The project splits so.
    Split(Kept),
    /// The project is whole.
    Whole,
}

impl Told {
    /// What telling `split` (none: whole) is.
    pub(super) fn of(split: Option<&Kept>) -> Self {
        split.map_or(Self::Whole, |kept| Self::Split(kept.clone()))
    }
}

/// Project `arranged`'s split as the layout keeps it: the tab it shows when that one is split,
/// else its first split tab; of its two panes, the one in focus is the one in use.
pub(super) fn kept_of(arranged: &super::Arranged) -> Option<Kept> {
    let split = |tab: &&super::Arranging| tab.panes.len() == 2;
    let shown = (arranged.tabs.iter()).filter(split).find(|tab| {
        arranged
            .shown
            .as_ref()
            .is_some_and(|shown| tab.panes.contains(shown))
    });
    let tab = shown.or_else(|| arranged.tabs.iter().find(split))?;
    let shown = tab.focused.clone().unwrap_or_else(|| tab.panes[0].clone());
    let at = tab.panes.iter().position(|pane| *pane == shown)?;
    let beside = tab.panes.get(1 - at)?.clone();
    Some(Kept {
        shown,
        beside,
        right: at == 0,
    })
}

/// Split `kept`'s tabs, left and right.
pub(super) fn sides(kept: &Kept) -> [String; 2] {
    match kept.right {
        true => [kept.shown.clone(), kept.beside.clone()],
        false => [kept.beside.clone(), kept.shown.clone()],
    }
}

/// What is beside tab `shown` when project's split `kept` comes back: the tab that was beside
/// when `shown` was in use; the one that was in use, on the other side, when `shown` was beside;
/// none for any other tab.
pub(super) fn again(kept: &Kept, shown: &str) -> Option<Beside> {
    let (key, right) = if shown == kept.shown {
        (&kept.beside, kept.right)
    } else if shown == kept.beside {
        (&kept.shown, !kept.right)
    } else {
        return None;
    };
    Some(Beside {
        key: Key::parse(key)?,
        right,
    })
}

/// Whether split `now` is split `kept` seen from its other half: the same two tabs on the same
/// sides, the other one in use.
pub(super) fn same(now: &Kept, kept: Option<&Kept>) -> bool {
    kept.is_some_and(|kept| sides(kept) == sides(now))
}

/// What a split window does as the layout tells its project's split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Follow {
    /// Nothing: the layout keeps what the window shows, or the person changed the window since
    /// and the layout is told of that instead.
    Stay,
    /// This tab is beside the one shown now.
    Beside(Beside),
    /// The window is whole now.
    Whole,
}

/// What the window, split as `now`, does now that the layout keeps `kept`, having kept `drawn`
/// when the window was last made one with it; `shown` is the tab in use. A window still as the
/// layout kept it follows what the layout keeps now: the app put another pane there, or took
/// one away. One the person changed since is theirs.
pub(super) fn followed(
    now: Option<&Kept>,
    drawn: Option<&Kept>,
    kept: Option<&Kept>,
    shown: &str,
) -> Follow {
    let Some(now) = now else {
        return Follow::Stay;
    };
    let as_drawn = drawn == Some(now) || same(now, drawn);
    let as_kept = kept == Some(now) || same(now, kept);
    if !as_drawn || as_kept {
        return Follow::Stay;
    }
    match kept.and_then(|kept| again(kept, shown)) {
        // The same two tabs the other way round is the person's swap, not the layout's.
        Some(beside) if beside.key.name() == now.beside => Follow::Stay,
        Some(beside) => Follow::Beside(beside),
        None => Follow::Whole,
    }
}

/// The space between the halves, in points: the window's glass, as a line.
pub(super) const GAP: f64 = 1.0;
/// The least either half keeps, in points, however the line is dragged or the window sized.
pub(super) const LEAST_POINTS: f64 = 190.0;

/// The width of the line between the halves that the person takes hold of, in points: the
/// drawn point and four either side of it.
pub(super) const LINE: f64 = 9.0;

/// The window split, or not, and how.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Split {
    pub beside: Option<Beside>,
    /// The left half's share of the width, as the person left it (a half until they drag).
    pub share: f64,
    /// The share when the person took hold of the line: a drag moves from it.
    pub grabbed: Option<f64>,
    /// A final size awaiting the layout; an older projection cannot snap this pair back.
    pub sizing: Option<([Key; 2], f64)>,
    /// What the window last told the layout of its project's split, until the layout tells it
    /// back: told twice is told once, and a split the person undid does not come back meanwhile.
    pub told: Told,
    /// The split the layout kept for the project shown (its key) when the window was last
    /// made one with it: what the layout changes since is the app's doing, and followed.
    pub drawn: Option<(String, Option<Kept>)>,
}

impl Default for Split {
    fn default() -> Self {
        Self {
            beside: None,
            share: 0.5,
            grabbed: None,
            sizing: None,
            told: Told::Nothing,
            drawn: None,
        }
    }
}

/// A share of `room` points held where both halves keep `LEAST_POINTS` (on a room too narrow
/// for both, they keep half each).
pub(super) fn held(share: f64, room: f64) -> f64 {
    let least = (LEAST_POINTS / room.max(1.0)).clamp(crate::layout::LEAST, 0.5);
    share.clamp(least, 1.0 - least)
}

impl Split {
    /// Follow saved proportions, except during a drag or before its final size is acknowledged.
    pub(super) fn follow_share(&mut self, pair: Option<(Key, Key)>, saved: Option<f64>) {
        let panes = pair.map(|(left, right)| [left, right]);
        if self
            .sizing
            .as_ref()
            .is_some_and(|(keys, _)| Some(*keys) != panes)
        {
            self.sizing = None;
        }
        if self.grabbed.is_some() {
            return;
        }
        let Some(saved) = saved.filter(|share| share.is_finite()) else {
            return;
        };
        if let Some((_, pending)) = self.sizing {
            if (saved - pending).abs() > 0.00011 {
                return;
            }
            self.sizing = None;
        }
        self.share = saved;
    }
}

/// The normalized left share for this exact pair, not another split in the project.
pub(super) fn share_of(arranged: &super::Arranged, (left, right): (Key, Key)) -> Option<f64> {
    let names = [left.name(), right.name()];
    let tab = arranged
        .tabs
        .iter()
        .find(|tab| tab.across && tab.panes == names)?;
    let [first, second] = tab.shares.as_slice() else {
        return None;
    };
    if !first.is_finite() || !second.is_finite() || *first <= 0.0 || *second <= 0.0 {
        return None;
    }
    let total = first + second;
    if !total.is_finite() {
        return None;
    }
    let share = first / total;
    (share > 0.0 && share < 1.0).then_some(share)
}

/// The halves of `width` points starting `left`, the first `share` of it: each one's left and
/// width.
pub(super) fn halves(width: f64, left: f64, share: f64) -> ((f64, f64), (f64, f64)) {
    let room = (width - GAP).max(0.0);
    let first = (room * share.clamp(0.0, 1.0)).round();
    let second = room - first;
    ((left, first), (left + first + GAP, second))
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
