//! Where the window's two shapes go on a screen: the expanded window on its right, the mini in a
//! corner, each where the person left it when they moved it. Pure, in points, from the top left.

/// The part of a screen windows may use (the menu bar and the Dock left out).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The smallest a window the person sees is made: its bar, and room for a page under it.
pub(super) const LEAST: tao::dpi::LogicalSize<f64> = tao::dpi::LogicalSize {
    width: 480.0,
    height: 192.0,
};

/// What the person moved or sized, kept for the process (host.rs's `Shared`).
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Kept {
    pub deck: Option<Rect>,
    pub mini: Option<(f64, f64)>,
    /// The tabs' column was left open or slid shut: a window opened again keeps it so.
    pub side: Option<bool>,
    /// The side's width as the person last dragged it.
    pub side_width: Option<f64>,
}

/// The mini's size: its picture, 16 by 10.
pub(super) const MINI: (f64, f64) = (288.0, 180.0);
/// The gap between the expanded window and the screen's edges.
const MARGIN: f64 = 12.0;
/// The gap between the mini and the screen's corner.
const CORNER: f64 = 16.0;

fn tens(value: f64) -> f64 {
    (value / 10.0).round() * 10.0
}

/// The expanded window: `kept` if the person set one, else about 60% of the width (no narrower
/// than a page needs, no wider than reads well) on the right, the height the screen has.
pub(super) fn deck(area: Area, kept: Option<Rect>) -> Rect {
    if let Some(kept) = kept {
        return Rect {
            width: kept.width.max(LEAST.width),
            height: kept.height.max(LEAST.height),
            ..kept
        };
    }
    let room = area.width - 2.0 * MARGIN;
    let width = tens(area.width * 0.6).clamp(960.0, 1440.0).min(room);
    Rect {
        x: area.x + area.width - width - MARGIN,
        y: area.y + MARGIN,
        width,
        height: (area.height - 2.0 * MARGIN).max(0.0),
    }
}

/// The place of the window `steps` steps after the main one (the first other window is 1):
/// `deck` moved down and right by a step each, as the system cascades new windows, so two never
/// lie on one another. Where the next step would leave `area`, it starts again from the area's
/// top left.
pub(super) fn cascade(area: Area, deck: Rect, steps: usize) -> Rect {
    const STEP: f64 = 28.0;
    let steps = f64::from(u32::try_from(steps).unwrap_or(u32::MAX));
    let along = |from: f64, start: f64, room: f64| {
        let room = room.max(0.0);
        let moved = from - start + STEP * steps;
        start + moved.rem_euclid(room + STEP).min(room)
    };
    Rect {
        x: along(deck.x, area.x, area.width - deck.width),
        y: along(deck.y, area.y, area.height - deck.height),
        ..deck
    }
}

/// The mini's top left: `kept` while it is on the screen, else the bottom right corner.
pub(super) fn mini(area: Area, kept: Option<(f64, f64)>) -> (f64, f64) {
    let (width, height) = MINI;
    let corner = (
        area.x + area.width - width - CORNER,
        area.y + area.height - height - CORNER,
    );
    let Some((x, y)) = kept else {
        return corner;
    };
    (
        x.clamp(area.x, (area.x + area.width - width).max(area.x)),
        y.clamp(area.y, (area.y + area.height - height).max(area.y)),
    )
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
