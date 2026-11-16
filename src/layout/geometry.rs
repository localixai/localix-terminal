//! Where a tab's panes lie: its tree laid over an area in points, the dividers between the
//! children of each branch, the pane beside another in a direction, whether a pane still
//! splits, and the step a key moves a divider. Plain maths over the tree, whatever names its
//! panes: the window lays its views out by it and the desk asks it what fits. It knows no
//! window.

use super::{LEAST, Share, Tree};

/// A rectangle in points, from the top left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether point `x`, `y` is in it (its right and bottom edges are its neighbour's).
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Where it starts along one way (`across`: left to right, else top to bottom).
    pub const fn start(&self, across: bool) -> f64 {
        if across { self.x } else { self.y }
    }

    /// How long it is along one way.
    pub const fn length(&self, across: bool) -> f64 {
        if across { self.width } else { self.height }
    }

    /// The part of it from `start` along one way, `length` long, as wide as it the other way.
    const fn along(&self, across: bool, start: f64, length: f64) -> Self {
        if across {
            Self::new(start, self.y, length, self.height)
        } else {
            Self::new(self.x, start, self.width, length)
        }
    }
}

/// A way to go from a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Up,
    Down,
}

impl Side {
    /// Whether it goes side to side.
    pub const fn across(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Whether it goes right or down, the way a branch lays its children.
    pub const fn forward(self) -> bool {
        matches!(self, Self::Right | Self::Down)
    }

    /// The way back.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

/// How a tree is laid out, in points: the space between two panes, and the least a pane keeps
/// where there is room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub gap: f64,
    pub least: f64,
}

/// The share of its branch a key moves a divider by, and with Shift.
pub const STEP: f64 = 0.05;
pub const STEP_FAR: f64 = 0.2;

/// The space between two children of a branch: the branch's place in the tree (`path`), the
/// child it comes before (`at`), the branch's way, the gap itself (as long as the branch is
/// the other way), and the points the branch shares out, which a drag along it is measured by.
#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    pub path: Vec<usize>,
    pub at: usize,
    pub across: bool,
    pub rect: Rect,
    pub room: f64,
}

/// A tree laid out: each pane in sight with its rectangle, in order, and every divider.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Laid<P> {
    pub panes: Vec<(P, Rect)>,
    pub dividers: Vec<Divider>,
}

impl<P: PartialEq> Laid<P> {
    /// The rectangle of pane `pane`, if it is in sight.
    pub fn rect(&self, pane: &P) -> Option<Rect> {
        (self.panes.iter())
            .find(|(it, _)| it == pane)
            .map(|(_, rect)| *rect)
    }
}

/// Tree `tree` laid over `area`.
pub fn lay<P: Clone + PartialEq>(tree: &Tree<P>, area: Rect, spacing: Spacing) -> Laid<P> {
    let mut laid = Laid {
        panes: vec![],
        dividers: vec![],
    };
    lay_into(tree, area, spacing, &mut vec![], &mut laid);
    laid
}

fn lay_into<P: Clone + PartialEq>(
    tree: &Tree<P>,
    area: Rect,
    spacing: Spacing,
    path: &mut Vec<usize>,
    laid: &mut Laid<P>,
) {
    match tree {
        Tree::Leaf { pane } | Tree::Stack { shown: pane, .. } => {
            laid.panes.push((pane.clone(), area));
        }
        Tree::Branch { across, children } => {
            let room = room_of(area, *across, children.len(), spacing.gap);
            let parts = parts(children, area, *across, spacing);
            for (at, (child, rect)) in children.iter().zip(parts).enumerate() {
                if at > 0 {
                    let start = rect.start(*across) - spacing.gap;
                    laid.dividers.push(Divider {
                        path: path.clone(),
                        at,
                        across: *across,
                        rect: area.along(*across, start, spacing.gap),
                        room,
                    });
                }
                path.push(at);
                lay_into(&child.tree, rect, spacing, path, laid);
                path.pop();
            }
        }
    }
}

/// The points a branch of `count` children shares out along its way: `area` less the gaps.
fn room_of(area: Rect, across: bool, count: usize, gap: f64) -> f64 {
    let gaps = gap * count.saturating_sub(1) as f64;
    (area.length(across) - gaps).max(0.0)
}

/// The children's parts of `area` along its way, a gap between each two. Each keeps the least
/// its panes need where there is room (`held`); every end is a whole point but the last, which
/// ends where the area does.
fn parts<P: Clone + PartialEq>(
    children: &[Share<P>],
    area: Rect,
    across: bool,
    spacing: Spacing,
) -> Vec<Rect> {
    let room = room_of(area, across, children.len(), spacing.gap);
    let needs: Vec<f64> = (children.iter())
        .map(|child| {
            let span = child.tree.span(across) as f64;
            span * spacing.least + (span - 1.0) * spacing.gap
        })
        .collect();
    let shares: Vec<f64> = children.iter().map(|child| child.share).collect();
    let shares = held(&shares, &needs, room);
    let (mut sum, mut from) = (0.0, 0.0);
    let last = shares.len().saturating_sub(1);
    let mut out = Vec::with_capacity(shares.len());
    for (at, share) in shares.iter().enumerate() {
        sum += share;
        let end = if at == last {
            room
        } else {
            (room * sum).round().clamp(from, room)
        };
        let start = area.start(across) + from + spacing.gap * at as f64;
        out.push(area.along(across, start, end - from));
        from = end;
    }
    out
}

/// Shares `shares` of `room` points held so that each keeps `needs` of it, and `LEAST` of the
/// branch at the very least: one under its least is given it, the rest share what remains in
/// their proportions. A room too narrow for all gives each a part by what it needs.
pub fn held(shares: &[f64], needs: &[f64], room: f64) -> Vec<f64> {
    let room = room.max(1.0);
    let least: Vec<f64> = needs.iter().map(|need| (need / room).max(LEAST)).collect();
    let all: f64 = least.iter().sum();
    if all >= 1.0 {
        return least.iter().map(|least| least / all).collect();
    }
    let mut fixed = vec![false; shares.len()];
    loop {
        let kept: f64 = (least.iter().zip(&fixed))
            .filter(|(_, fixed)| **fixed)
            .map(|(least, _)| least)
            .sum();
        let free: f64 = (shares.iter().zip(&fixed))
            .filter(|(_, fixed)| !**fixed)
            .map(|(share, _)| share.max(0.0))
            .sum();
        let count = fixed.iter().filter(|fixed| !**fixed).count().max(1) as f64;
        let given = |share: f64| match free > 0.0 {
            true => share.max(0.0) / free * (1.0 - kept),
            false => (1.0 - kept) / count,
        };
        let short = (0..shares.len()).find(|at| !fixed[*at] && given(shares[*at]) < least[*at]);
        match short {
            Some(at) => fixed[at] = true,
            None => {
                let out = (0..shares.len()).map(|at| match fixed[at] {
                    true => least[at],
                    false => given(shares[at]),
                });
                return out.collect();
            }
        }
    }
}

/// How far rectangle `rect` lies alongside `here` across the way `across` (negative: apart).
fn alongside(here: &Rect, rect: &Rect, across: bool) -> f64 {
    let end = here.start(!across) + here.length(!across);
    let to = rect.start(!across) + rect.length(!across);
    to.min(end) - rect.start(!across).max(here.start(!across))
}

/// Every pane beside pane `from` on `side`, among `panes` laid out `gap` apart, in their
/// order: those whose near edge meets `from`'s on that side and that lie alongside it. None at
/// the tab's edge, or for a pane not among them.
pub fn beside<P: Clone + PartialEq>(panes: &[(P, Rect)], from: &P, side: Side, gap: f64) -> Vec<P> {
    let Some(here) = panes
        .iter()
        .find(|(pane, _)| pane == from)
        .map(|(_, rect)| *rect)
    else {
        return vec![];
    };
    let across = side.across();
    let edge = here.start(across)
        + if side.forward() {
            here.length(across)
        } else {
            0.0
        };
    let meets = |rect: &Rect| {
        let apart = match side.forward() {
            true => rect.start(across) - edge,
            false => edge - (rect.start(across) + rect.length(across)),
        };
        (-0.5..=gap + 0.5).contains(&apart)
    };
    (panes.iter())
        .filter(|(pane, rect)| pane != from && meets(rect) && alongside(&here, rect, across) > 0.0)
        .map(|(pane, _)| pane.clone())
        .collect()
}

/// The pane beside pane `from` on `side` (`beside`): the one used most lately (`recent`, the
/// latest first), then the one alongside it most, then the one nearest its middle. None at the
/// tab's edge: nothing wraps round.
pub fn neighbour<P: Clone + PartialEq>(
    panes: &[(P, Rect)],
    from: &P,
    side: Side,
    recent: &[P],
    gap: f64,
) -> Option<P> {
    let here = panes.iter().find(|(pane, _)| pane == from)?.1;
    let across = side.across();
    let rect = |pane: &P| {
        (panes.iter())
            .find(|(it, _)| it == pane)
            .map_or(here, |(_, rect)| *rect)
    };
    let middle = |rect: &Rect| rect.start(!across) + rect.length(!across) / 2.0;
    let centre = middle(&here);
    let lately = |pane: &P| {
        recent
            .iter()
            .position(|it| it == pane)
            .unwrap_or(usize::MAX)
    };
    (beside(panes, from, side, gap).into_iter()).min_by(|one, other| {
        let (a, b) = (rect(one), rect(other));
        (lately(one).cmp(&lately(other)))
            .then(alongside(&here, &b, across).total_cmp(&alongside(&here, &a, across)))
            .then(
                (middle(&a) - centre)
                    .abs()
                    .total_cmp(&(middle(&b) - centre).abs()),
            )
    })
}

/// Whether pane `pane`, laid out in `laid`, splits along one way (`across`: side by side) with
/// both parts keeping the least a pane keeps.
pub fn fits<P: PartialEq>(laid: &Laid<P>, pane: &P, across: bool, spacing: Spacing) -> bool {
    (laid.rect(pane)).is_some_and(|rect| rect.length(across) >= 2.0 * spacing.least + spacing.gap)
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
