//! Tests of `presence`.

use super::Effect::{Busy, Enter};
use super::Event::{Acting, Appeared, Collapse, Expand, Settled, Vanished};
use super::{Presence, State, Support};

fn macos() -> Presence {
    Presence::new(Support::Away)
}

/// A presence whose tabs appeared: the window is on screen.
fn open() -> Presence {
    let mut presence = macos();
    presence.event(Appeared);
    presence
}

#[test]
fn tabs_open_as_the_window_and_nothing_the_agent_does_hides_it() {
    let mut presence = macos();
    assert_eq!(presence.state(), State::Hidden);
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    assert_eq!(presence.state(), State::Expanded);
    for event in [Acting, Settled, Appeared] {
        presence.event(event);
        assert_eq!(presence.state(), State::Expanded, "{event:?}");
    }
}

#[test]
fn the_person_puts_the_window_away_and_only_they_bring_it_back() {
    let mut presence = open();
    assert_eq!(presence.event(Collapse), vec![Enter(State::Away)]);
    assert_eq!(presence.state(), State::Away);
    // Away, nothing is on screen, and nothing the agent does puts the window there (its spinner
    // is the turn's, which the strip says when the window is back).
    for event in [Acting, Settled, Appeared, Collapse] {
        let out = presence.event(event);
        assert!(
            out.iter().all(|effect| matches!(effect, Busy(_))),
            "{event:?}: {out:?}"
        );
        assert_eq!(presence.state(), State::Away, "{event:?}");
    }
    // Their click on the app (its Dock icon, `llx-desk open`) brings the window back.
    assert_eq!(presence.event(Expand), vec![Enter(State::Expanded)]);
    assert_eq!(presence.event(Expand), vec![]);
}

#[test]
fn without_putting_away_the_window_stays_an_ordinary_one() {
    let mut presence = Presence::new(Support::WindowOnly);
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    // Its minimize button is the system's own.
    assert_eq!(presence.event(Collapse), vec![]);
    assert_eq!(presence.state(), State::Expanded);
}

#[test]
fn a_spinner_while_the_agent_acts() {
    let mut presence = open();
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
    assert_eq!(presence.event(Acting), vec![]);
    assert_eq!(presence.event(Settled), vec![Busy(false)]);
    assert_eq!(presence.event(Settled), vec![]);
}

#[test]
fn a_window_put_away_with_no_tab_left_is_gone_and_starts_over() {
    let mut presence = open();
    presence.event(Collapse);
    presence.event(Acting);
    assert_eq!(presence.event(Vanished), vec![Enter(State::Hidden)]);
    assert_eq!(presence.event(Vanished), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Expand), vec![], "nothing to bring back");
    // A later window opens as the window again, with no spinner left over.
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
}
