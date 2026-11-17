//! Tests of `motion`: the native timings are the token sheet's, and the springs move as the
//! rules say.

use super::{ROOM, SETTLE, T_FADE, T_IN, spring, travelled};

/// The value of token `name` in `frame.css`, in seconds.
fn token(name: &str) -> f64 {
    let sheet = include_str!("frame.css");
    let (_, value) = sheet
        .split_once(&format!("{name}:"))
        .unwrap_or_else(|| panic!("no {name}"));
    let (value, _) = value.split_once('s').unwrap();
    value.parse().unwrap()
}

#[test]
fn the_native_timings_are_the_token_sheets() {
    assert!((token("--t-in") - T_IN).abs() < 1e-9);
    assert!((token("--t-fade") - T_FADE).abs() < 1e-9);
    // The sheet's springs are played over about a quarter of a second, as these are.
    let sheet = include_str!("frame.css");
    assert!(sheet.contains("about a quarter of a second"));
    assert!((SETTLE - 0.25).abs() < 1e-9);
}

#[test]
fn a_spring_with_no_bounce_never_turns_back_and_is_at_rest_when_it_has_played() {
    let room = spring(ROOM);
    let mut last = 0.0;
    for step in 0..=1000 {
        let now = travelled(room, f64::from(step) * SETTLE / 1000.0);
        assert!(
            now >= last - 1e-12,
            "turned back at step {step}: {now} < {last}"
        );
        assert!(now <= 1.0 + 1e-12, "past its end at step {step}: {now}");
        last = now;
    }
    assert!(last > 0.998, "{last}");
    // It has visibly arrived (90%) at about 0.62 of its perceptual duration.
    let perceptual = SETTLE / 1.47;
    let arrived = travelled(room, 0.62 * perceptual);
    assert!((0.85..0.95).contains(&arrived), "{arrived}");
}
