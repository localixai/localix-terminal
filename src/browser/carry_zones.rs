//! Where a tab carried over a window's panes lands, from the pointer alone. The zones are the
//! panes as the layout keeps them when the tab comes over them, taken once: nothing that moves
//! while it is carried (the panes making room for it) changes them. A pane's left half is its
//! left and its right half its right, so the tab stands under the pointer; over it and under
//! it are taken near the pane's upper and lower edge only, in a zone as wide as the pane on
//! the edge that narrows to nothing a quarter of the pane's height in, so a pointer low in a
//! half is still that half. No place in a pane lands nowhere. A hand that shakes changes
//! nothing: another side is taken only once the pointer is in its zone by a margin, another
//! pane only once it is past the divider by one. What a landing makes of the panes is worked
//! out once for each pane and side, and kept while the tab is carried.

use super::Side;
use crate::layout::geometry::Rect;

/// How far in from a pane's upper or lower edge that edge's zone reaches, as a part of how
/// far the pointer is from the nearer of the pane's left and right, both as parts of the
/// pane: at the middle of the pane's width (a half from either), a quarter of its height.
const EDGE: f64 = 0.5;

/// The least margin, in points: how far into another side's zone the pointer goes before
/// that side is taken, and how far past a divider before another pane is.
const MARGIN: f64 = 8.0;

/// The margin as a part of a pane's shorter side, where that is more than `MARGIN`.
const MARGIN_PART: f64 = 0.05;

/// The zones of one window's panes for one carried tab, the target the pointer took, and the
/// landings worked out so far (`L`, of a pane `P` and a side).
pub(super) struct Zones<P, L> {
    panes: Vec<(P, Rect)>,
    area: Rect,
    target: Option<(P, Side)>,
    plans: Vec<((P, Side), Option<L>)>,
}

impl<P: Copy + PartialEq, L> Zones<P, L> {
    /// The zones of `panes` (each where the window lays it now) over the panes' `area`.
    pub(super) const fn new(panes: Vec<(P, Rect)>, area: Rect) -> Self {
        Self {
            panes,
            area,
            target: None,
            plans: vec![],
        }
    }

    /// The pane and side the tab lands on now, none where it lands nowhere.
    #[cfg(test)]
    pub(super) const fn target(&self) -> Option<(P, Side)> {
        self.target
    }

    /// The pointer at `x`, `y` of the window: whether that changed the target.
    pub(super) fn at(&mut self, x: f64, y: f64) -> bool {
        let was = self.target;
        self.target = self.aim(x, y);
        self.target != was
    }

    /// The target from the pointer at `x`, `y`, the one taken before kept where nothing says
    /// otherwise: in the space between two panes, within the margin of where two zones meet.
    fn aim(&self, x: f64, y: f64) -> Option<(P, Side)> {
        let area = self.area;
        if x < area.x || y < area.y || x > area.x + area.width || y > area.y + area.height {
            return None;
        }
        // Within the margin past its edge, the pane taken is still the one under the pointer.
        let kept = self.target.and_then(|(pane, _)| {
            let rect = self.rect(pane)?;
            grown(rect, MARGIN).contains(x, y).then_some((pane, rect))
        });
        let under = kept.or_else(|| {
            (self.panes.iter())
                .find(|(_, rect)| rect.contains(x, y))
                .copied()
        });
        let Some((pane, rect)) = under else {
            return self.target;
        };
        let was = self.target.filter(|(was, _)| *was == pane);
        Some((pane, side_at(rect, x, y, was.map(|(_, side)| side))))
    }

    /// Where pane `pane` lay.
    fn rect(&self, pane: P) -> Option<Rect> {
        (self.panes.iter())
            .find(|(it, _)| *it == pane)
            .map(|(_, rect)| *rect)
    }

    /// The landing at the target now, worked out by `make` the first time it is asked; none
    /// where the tab lands nowhere, or where landing there changes nothing.
    pub(super) fn plan(&mut self, make: impl FnOnce(P, Side) -> Option<L>) -> Option<&L> {
        let target = self.target?;
        let at = match self.plans.iter().position(|(it, _)| *it == target) {
            Some(at) => at,
            None => {
                self.plans.push((target, make(target.0, target.1)));
                self.plans.len() - 1
            }
        };
        self.plans.get(at)?.1.as_ref()
    }
}

/// `rect` grown by `by` on every side.
fn grown(rect: Rect, by: f64) -> Rect {
    Rect::new(
        rect.x - by,
        rect.y - by,
        rect.width + 2.0 * by,
        rect.height + 2.0 * by,
    )
}

/// The side of `rect` a tab at `x`, `y` lands on, `was` the side taken there before (none: the
/// tab has just come over the pane). Fresh, the pointer's zone; else `was`, until the pointer
/// is in another side's zone by the margin.
fn side_at(rect: Rect, x: f64, y: f64, was: Option<Side>) -> Side {
    let across = ((x - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0);
    let down = ((y - rect.y) / rect.height.max(1.0)).clamp(0.0, 1.0);
    // How far into the zone of the nearer of the upper and lower edge the pointer is, in
    // points down the pane (under nothing: that far out of it), and how far right of the
    // line between the pane's halves.
    let into = (EDGE * across.min(1.0 - across) - down.min(1.0 - down)) * rect.height;
    let right = (across - 0.5) * rect.width;
    let edge = if down < 0.5 { Side::Up } else { Side::Down };
    let half = if right < 0.0 { Side::Left } else { Side::Right };
    let margin = MARGIN.max(MARGIN_PART * rect.width.min(rect.height));
    match was {
        None if into > 0.0 => edge,
        None => half,
        Some(_) if into > margin => edge,
        Some(Side::Up | Side::Down) if into < -margin => half,
        Some(Side::Left) if right > margin => half,
        Some(Side::Right) if right < -margin => half,
        Some(was) => was,
    }
}

#[cfg(test)]
#[path = "carry_zones_tests.rs"]
mod tests;
