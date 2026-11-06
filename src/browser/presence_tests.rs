//! Tests of `presence`.

use std::time::Duration;

use super::Effect::{Blink, Busy, Capture, Enter};
use super::Event::{
    Acting, Appeared, Changed, Collapse, Expand, Pictured, Respaced, Settled, Show, Vanished,
};
use super::{Presence, State, Support, Timing};

fn mini() -> Presence {
    Presence::new(Support::Mini, Timing::default())
}

/// A presence whose tabs appeared, collapsed by the person to the mini, its first picture taken.
fn showing() -> Presence {
    let mut presence = mini();
    presence.event(Appeared);
    presence.event(Collapse);
    presence.event(Pictured);
    presence
}

#[test]
fn tabs_open_as_the_window_and_the_mini_only_when_asked() {
    let mut presence = mini();
    assert_eq!(presence.state(), State::Hidden);
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    assert_eq!(presence.state(), State::Expanded);
    // Nothing the agent does makes the mini.
    for event in [Acting, Show, Changed, Settled, Pictured, Respaced] {
        presence.event(event);
        assert_eq!(presence.state(), State::Expanded, "{event:?}");
    }
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
}

#[test]
fn without_a_mini_the_window_opens_ordinary() {
    let mut presence = Presence::new(Support::WindowOnly, Timing::default());
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    // Nothing collapses it, and no picture is kept.
    assert_eq!(presence.event(Collapse), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.state(), State::Expanded);
}

#[test]
fn only_the_person_expands_and_collapses() {
    let mut presence = showing();
    assert_eq!(presence.event(Expand), vec![Enter(State::Expanded)]);
    assert_eq!(presence.event(Expand), vec![]);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
    assert_eq!(presence.event(Collapse), vec![]);
}

#[test]
fn the_agent_never_expands_the_window() {
    let mut presence = showing();
    for event in [Acting, Show, Changed, Settled, Pictured] {
        let out = presence.event(event);
        assert!(!out.contains(&Enter(State::Expanded)), "{event:?}: {out:?}");
        assert_eq!(presence.state(), State::Mini, "{event:?}");
    }
}

#[test]
fn show_blinks_the_mini_and_takes_a_new_picture() {
    let mut presence = showing();
    let blink = Timing::default().blink;
    assert_eq!(presence.event(Show), vec![Blink(blink), Capture]);
    // Expanded, the person sees the tab itself: nothing to blink.
    presence.event(Expand);
    assert_eq!(presence.event(Show), vec![]);
}

#[test]
fn a_spinner_while_the_agent_acts_and_a_picture_when_it_settles() {
    let mut presence = showing();
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
    assert_eq!(presence.event(Acting), vec![]);
    assert_eq!(presence.event(Settled), vec![Busy(false), Capture]);
    // The picture asked for is still being taken: the next one waits for it.
    assert_eq!(presence.event(Settled), vec![]);
}

#[test]
fn pictures_are_taken_one_at_a_time_and_the_newest_follows() {
    let mut presence = mini();
    presence.event(Appeared);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
    // Two changes while a picture is in flight: one more is taken, not two.
    assert_eq!(presence.event(Changed), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Pictured), vec![Capture]);
    assert_eq!(presence.event(Pictured), vec![]);
}

#[test]
fn expanded_it_takes_no_pictures_and_collapsing_takes_one() {
    let mut presence = showing();
    presence.event(Expand);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Changed), vec![]);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
}

#[test]
fn no_tabs_hides_everything_and_starts_over() {
    let mut presence = showing();
    presence.event(Acting);
    assert_eq!(presence.event(Vanished), vec![Enter(State::Hidden)]);
    assert_eq!(presence.event(Vanished), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    // A later window opens as the window again, with no spinner left over.
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
}

#[test]
fn a_blink_is_as_long_as_it_is_told() {
    let timing = Timing {
        blink: Duration::from_millis(5),
    };
    let mut presence = Presence::new(Support::Mini, timing);
    presence.event(Appeared);
    presence.event(Collapse);
    presence.event(Pictured);
    assert_eq!(
        presence.event(Show),
        vec![Blink(Duration::from_millis(5)), Capture]
    );
}

#[test]
fn another_space_shows_the_mini_again_and_nothing_else() {
    let mut presence = mini();
    // Hidden: nothing to show.
    assert_eq!(presence.event(Respaced), vec![]);
    presence.event(Appeared);
    presence.event(Collapse);
    assert_eq!(presence.event(Respaced), vec![Enter(State::Mini)]);
    assert_eq!(presence.state(), State::Mini);
    // Expanded: the deck moves to the person's Space on its own.
    presence.event(Expand);
    assert_eq!(presence.event(Respaced), vec![]);
    assert_eq!(presence.state(), State::Expanded);
}
