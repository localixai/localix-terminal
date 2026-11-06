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

/// A project's split as kept while the window shows another: the tab in use, the one beside,
/// by their keys (`w1`, `t2`), and whether that one is on the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Kept {
    pub shown: String,
    pub beside: String,
    pub right: bool,
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

/// The space between the halves, in points: the window's glass, as a line.
pub(super) const GAP: f64 = 1.0;
/// The least share of the width either half keeps while the line between them is dragged.
pub(super) const LEAST: f64 = 0.2;

/// The width of the line between the halves that the person takes hold of, in points.
pub(super) const LINE: f64 = 7.0;

/// The window split, or not, and how.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Split {
    pub beside: Option<Beside>,
    /// The left half's share of the width, as the person left it (a half until they drag).
    pub share: f64,
    /// The share when the person took hold of the line: a drag moves from it.
    pub grabbed: Option<f64>,
}

impl Default for Split {
    fn default() -> Self {
        Self {
            beside: None,
            share: 0.5,
            grabbed: None,
        }
    }
}

/// A share held where both halves keep some of the width.
pub(super) fn held(share: f64) -> f64 {
    share.clamp(LEAST, 1.0 - LEAST)
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
