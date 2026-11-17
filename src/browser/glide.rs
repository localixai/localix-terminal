//! The window's views moved by a layout, as the person sees them move (docs/ui.md, Motion). A
//! layout puts each view where it goes at once, and never more than once; what is drawn of a
//! view that moved goes there from where it stood, by a spring the system plays on the view's
//! own layer: its place and how much of it shows, never its size on the way, so what a pane
//! holds keeps its place from its top left and is cut at its edge, as while a divider is held.
//! A spring is added to those still playing, so a new layout takes the views on from where
//! they are and as fast as they go. A pane that comes into sight where a carried tab would
//! land opens from the edge it lands at, in step with the pane that makes room for it.
//!
//! Here is the geometry, the same wherever the views are; `glide_macos.rs` plays it.

use super::Side;

#[cfg(target_os = "macos")]
#[path = "glide_macos.rs"]
mod native;
#[cfg(not(target_os = "macos"))]
#[path = "glide_other.rs"]
mod native;

#[cfg(target_os = "macos")]
pub(super) use native::drawn;
pub(super) use native::standing;

/// A view's frame in its parent's points, as the parent counts them: across, along its other
/// way (from its top, or from its bottom), wide and tall.
#[cfg(any(target_os = "macos", test))]
pub(super) type Frame = [f64; 4];

/// A view of the window that a layout moves.
#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "the views macOS moves; here they are only named")
)]
#[derive(Clone, Copy)]
pub(super) enum Moved<'a> {
    Term(&'a super::term_view::TermView),
    Page(&'a wry::WebView),
}

/// The pane that opens where a carried tab would land: its views, and the side of the pane
/// beside it that it lands on (so the edge of its own it opens from).
#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "what macOS opens a pane by; here nothing moves")
)]
pub(super) struct Opens<'a> {
    pub views: &'a [Moved<'a>],
    pub side: Side,
    /// The divider between it and the pane beside it, and how far that pane's edge travels to
    /// make room: the divider comes with the edge.
    pub line: Option<(&'a wry::WebView, f64)>,
}

/// For checks: where a view goes and where it is drawn now (points from its parent's top
/// left: across, down, wide, tall), where the top left of what it holds is drawn and how far
/// up from there what it holds is drawn raised (a terminal keeping its cursor's row in a pane
/// grown shorter), how clear it is drawn (1: not at all), how many of the springs and fades
/// added here it still plays, and whether it is in sight.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drawn {
    pub goes: Frame,
    pub now: Frame,
    pub holds: (f64, f64),
    pub raised: f64,
    pub opacity: f32,
    pub playing: usize,
    pub seen: bool,
}

/// `frame` shut onto its own edge on `side`: where a pane that opens from that edge starts.
/// `from_top` says its parent counts from its top.
#[cfg(any(target_os = "macos", test))]
pub(super) fn shut(frame: Frame, side: Side, from_top: bool) -> Frame {
    let [x, y, wide, tall] = frame;
    // The parent's count along its other way grows toward the frame's bottom or its top.
    let (top, bottom) = if from_top {
        (y, y + tall)
    } else {
        (y + tall, y)
    };
    match side {
        Side::Left => [x, y, 0.0, tall],
        Side::Right => [x + wide, y, 0.0, tall],
        Side::Up => [x, top, wide, 0.0],
        Side::Down => [x, bottom, wide, 0.0],
    }
}

/// `frame` moved `by` points toward `side`.
#[cfg(any(target_os = "macos", test))]
pub(super) fn toward(frame: Frame, side: Side, by: f64, from_top: bool) -> Frame {
    let [x, y, wide, tall] = frame;
    let down = if from_top { by } else { -by };
    match side {
        Side::Left => [x - by, y, wide, tall],
        Side::Right => [x + by, y, wide, tall],
        Side::Up => [x, y - down, wide, tall],
        Side::Down => [x, y + down, wide, tall],
    }
}

/// How far from where it goes a layer is drawn as its spring starts: its place in its parent,
/// where its bounds begin, and their size. Each goes to nothing as the spring plays. The size
/// is apart from where the bounds begin: a rectangle of a width under nothing is not one the
/// system keeps as given.
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Offset {
    pub place: (f64, f64),
    pub begin: (f64, f64),
    pub size: (f64, f64),
}

#[cfg(any(target_os = "macos", test))]
impl Offset {
    /// Whether it is none: the view stands where it stood.
    pub(super) fn none(&self) -> bool {
        let Self { place, begin, size } = *self;
        [place, begin, size]
            .iter()
            .all(|(across, down)| across.abs() < 0.01 && down.abs() < 0.01)
    }
}

/// The offset that draws a view's layer over `before` when the view stands at `after`. The
/// layer is placed by its `anchor` (a part of its size from its frame's origin); `from_top`
/// says what it holds is counted from its top, else from its bottom, where its bounds begin
/// as much lower as it is taller, so that what it holds keeps its place from its top.
#[cfg(any(target_os = "macos", test))]
pub(super) fn offset(before: Frame, after: Frame, anchor: (f64, f64), from_top: bool) -> Offset {
    let (wider, taller) = (before[2] - after[2], before[3] - after[3]);
    Offset {
        place: (
            before[0] - after[0] + anchor.0 * wider,
            before[1] - after[1] + anchor.1 * taller,
        ),
        begin: (0.0, if from_top { 0.0 } else { -taller }),
        size: (wider, taller),
    }
}

#[cfg(test)]
#[path = "glide_tests.rs"]
mod tests;
