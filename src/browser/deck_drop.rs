//! Tab drop hit testing, split previews and the draggable split seam.

use super::super::strip::Key;
use super::{Deck, Landing, Over};

/// A whole window's middle is not a split target.
const DEAD_MIDDLE: f64 = 0.18;

impl Deck {
    /// The tab in the half at `x`, `y` of the window (points from its top left), when the window
    /// is split and the press is on a page.
    pub(in super::super) fn half_at(&self, x: f64, y: f64) -> Option<Key> {
        let (size, left, top) = self.area(false);
        if y < top {
            return None;
        }
        let halves = self.halves(size.width, left)?;
        // A press on the line takes hold of it: the half beside it stays as it is.
        let seam = halves[1].1 - super::super::split::GAP / 2.0;
        if (x - seam).abs() <= super::super::split::LINE / 2.0 {
            return None;
        }
        (halves.into_iter())
            .find(|&(_, from, width)| x >= from && x < from + width)
            .map(|half| half.0)
    }

    /// Where `x`, `y` of the window is for a tab carried there: a split window's own half, a
    /// whole one's even halves (its middle none), or off the pages.
    pub(in super::super) fn over(&self, x: f64, y: f64) -> Over {
        let (size, left, top) = self.area(false);
        // Off the pages, past the window's edges too, it goes nowhere.
        if x < left || y < top || x > left + size.width || y > top + size.height {
            return Over::Off;
        }
        let Some([_, second]) = self.halves(size.width, left) else {
            // A whole window splits only for a tab taken well to one side: its middle is no half.
            let off = (x - (left + size.width / 2.0)) / size.width.max(1.0);
            return if off.abs() < DEAD_MIDDLE {
                Over::Middle
            } else {
                Over::Half(off > 0.0)
            };
        };
        Over::Half(x >= second.1)
    }

    /// Tab `key` carried over the pages at `at` (a point of the window), or off them (none):
    /// the window is laid out as it will be where the tab lands, the halves as they will be, and
    /// back as it was off the pages. Over a whole window's middle it stays as it is.
    pub(in super::super) fn drop_over(&mut self, key: Key, at: Option<(f64, f64)>) {
        let over = at.map_or(Over::Off, |(x, y)| self.over(x, y));
        let side = match over {
            Over::Middle => return,
            Over::Off => None,
            Over::Half(right) => Some(right),
        };
        if side == self.drop {
            return;
        }
        self.drop = side;
        // Where it lands is worked out from the window as it was before the tab came over it.
        let was = self
            .landing
            .map_or(self.split.beside, |landing| landing.was);
        let landed = side.and_then(|right| {
            super::super::split::landed(was, self.shown_key(), self.pair_with(was), key, right)
        });
        match landed {
            Some(beside) => {
                self.landing.get_or_insert(Landing { was });
                self.split.beside = Some(beside);
                self.split.grabbed = None;
            }
            None => {
                if let Some(landing) = self.landing.take() {
                    self.split.beside = landing.was;
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
            self.split.beside = landing.was;
            self.layout();
        }
    }

    /// The tab carried over the pages let go of: laid out where it landed, it stays so (true);
    /// else nothing was laid out (false).
    pub(in super::super) fn land(&mut self) -> bool {
        self.drop = None;
        self.landing.take().is_some()
    }

    /// The line between the halves dragged `moved` points since the person took hold of it; let
    /// go of when `done`.
    pub(in super::super) fn drag_split(&mut self, moved: i32, done: bool) {
        let (size, ..) = self.area(false);
        let room = (size.width - super::super::split::GAP).max(1.0);
        let from = *self.split.grabbed.get_or_insert(self.split.share);
        self.split.share = super::super::split::held(from + f64::from(moved) / room, room);
        if done {
            self.split.grabbed = None;
        }
        self.layout();
    }
}
