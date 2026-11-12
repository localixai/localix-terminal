//! The kinds are found by name and by their keys' letter, and a thread sees only its own.

use super::testing::{ECHO, Echo};
use super::*;

#[test]
fn a_kind_is_found_by_its_name_and_by_its_letter() {
    let _echo = Echo::installed();
    assert_eq!(named("echo").map(|kind| kind.about().title), Some("Echo"));
    assert_eq!(lettered('x').map(|(at, _)| at), Some(0));
    assert!(named("board").is_none());
    assert!(lettered('w').is_none());
    assert_eq!(abouts().len(), 1);
}

#[test]
fn a_thread_that_listed_none_has_none() {
    let _echo = Echo::installed();
    let other = std::thread::Builder::new().spawn(|| all().len()).unwrap();
    let other = other.join().unwrap_or(9);
    assert_eq!(other, 0);
}

#[test]
fn a_kind_whose_keys_would_be_another_tabs_is_not_listed() {
    static SETTINGS: About = About {
        id: "shop",
        letter: 's',
        ..copied()
    };
    static TWICE: About = About {
        id: "again",
        ..copied()
    };
    static LOUD: About = About {
        id: "loud",
        letter: 'Q',
        ..copied()
    };
    let listed = distinct(vec![
        Echo::of(&SETTINGS),
        Echo::of(&ECHO),
        Echo::of(&TWICE),
        Echo::of(&LOUD),
    ]);
    let ids: Vec<_> = listed.iter().map(|kind| kind.about().id).collect();
    assert_eq!(
        ids,
        ["echo"],
        "s1 is the settings', x is taken once, Q is no key's letter"
    );
}

/// The echo's own description, for a test to change a field of.
const fn copied() -> About {
    About {
        id: ECHO.id,
        letter: ECHO.letter,
        title: ECHO.title,
        holds: ECHO.holds,
        glyph: ECHO.glyph,
        fields: ECHO.fields,
        guide: ECHO.guide,
        instead: ECHO.instead,
        closed: ECHO.closed,
        keys: ECHO.keys,
    }
}
