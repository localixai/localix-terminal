//! The window's own timings for what native code moves (docs/ui.md, Motion): the same tokens
//! the pages take from `frame.css`, so a native motion and a page's never differ, and the
//! springs as Core Animation plays them. A spring is a damped one of mass 1 with no starting
//! speed: its stiffness and damping come from its perceptual duration and its bounce, and it
//! plays over its settling time, about a quarter of a second, as the sheet's springs do.

/// An entrance's fade (`--t-in`) and a cross-fade (`--t-fade`), in seconds.
pub(super) const T_IN: f64 = 0.10;
pub(super) const T_FADE: f64 = 0.15;

/// How long a spring plays: its settling time, the duration the sheet's springs are used for.
pub(super) const SETTLE: f64 = 0.25;

/// The bounce of what makes room or opens from an anchor (`--spring`): none.
pub(super) const ROOM: f64 = 0.0;

/// A spring as Core Animation takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Spring {
    pub mass: f64,
    pub stiffness: f64,
    pub damping: f64,
    pub duration: f64,
}

/// The settling time over the perceptual duration: 1.47 at bounce 0, 1.39 at 0.15.
fn settling(bounce: f64) -> f64 {
    1.47 - (1.47 - 1.39) * (bounce / 0.15)
}

/// The spring of `bounce` (0 none, 0.15 a little) that settles in `SETTLE`.
pub(super) fn spring(bounce: f64) -> Spring {
    let perceptual = SETTLE / settling(bounce);
    let omega = std::f64::consts::TAU / perceptual;
    Spring {
        mass: 1.0,
        stiffness: omega * omega,
        damping: 2.0 * omega * (1.0 - bounce),
        duration: SETTLE,
    }
}

/// How far along its travel `spring` is `t` seconds in (0 at the start, 1 at rest), with no
/// speed at the start: what Core Animation plays, worked out, for checks.
#[cfg(test)]
pub(super) fn travelled(spring: Spring, t: f64) -> f64 {
    let omega = (spring.stiffness / spring.mass).sqrt();
    let zeta = spring.damping / (2.0 * (spring.stiffness * spring.mass).sqrt());
    if zeta >= 1.0 - 1e-9 {
        return 1.0 - (1.0 + omega * t) * (-omega * t).exp();
    }
    let damped = omega * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * omega * t).exp();
    1.0 - decay * ((damped * t).cos() + zeta * omega / damped * (damped * t).sin())
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;
