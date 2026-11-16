//! Tests of `term`: a terminal opens while the app has room for it; over the bound it is refused,
//! and no terminal whose program runs is ended to make room.

use serde_json::json;

use super::super::tests::{caller, desk};
use crate::browser::Heard;
use crate::testing::Scratch;

#[test]
fn a_terminal_over_the_bound_is_refused_and_no_running_program_is_ended_for_it() {
    let (desk, scratch) = (desk(), Scratch::new());
    desk.room.hold(2);
    let mine = caller(&desk, scratch.path(), None);
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200});
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    // The person closes t1's tab: its program runs on, out of sight.
    desk.heard("t1", Heard::Closed("closed"));
    // An `open` says what it did not open in its answer, as it does of any tab.
    let refused = desk.window(&mine.call(), &open).unwrap();
    assert!(
        refused.starts_with("not opened: 2 terminals are open") && refused.contains("`close`"),
        "{refused}"
    );
    let sessions: Vec<_> = crate::lock(&desk.sessions).values().cloned().collect();
    assert_eq!(sessions.len(), 2, "nothing was taken for the new one");
    assert!(
        sessions.iter().all(|session| !session.ended()),
        "no program was ended to make room"
    );
    // Room made by its owner's close: the next one opens, and the refusal is no longer news.
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t2"}))
        .unwrap();
    desk.window(&mine.call(), &open).unwrap();
    assert!(desk.room.refused().is_empty());
    desk.shutdown();
}

#[test]
fn a_terminal_whose_program_ended_makes_room_for_the_next_one() {
    let (desk, scratch) = (desk(), Scratch::new());
    desk.room.hold(3);
    let mine = caller(&desk, scratch.path(), None);
    let call = crate::desk::Call {
        person: true,
        ..mine.call()
    };
    let group = crate::projects::key(mine.project.id);
    for _ in 0..3 {
        desk.person_terminal(&call, &group, "sh -c 'exit 0'")
            .unwrap();
    }
    // Each program ends; waiting for text that never shows returns when it does.
    let sessions: Vec<_> = crate::lock(&desk.sessions).values().cloned().collect();
    for session in sessions {
        let until = session.wait(Some("never shown"), 0, std::time::Duration::from_secs(10));
        assert_eq!(until, crate::terminal::Until::Exited);
    }
    desk.person_terminal(&call, &group, "sh -c 'exit 0'")
        .unwrap();
    assert_eq!(crate::lock(&desk.sessions).len(), 3);
    assert!(desk.room.refused().is_empty());
    desk.shutdown();
}

#[test]
fn another_projects_terminals_are_never_ended_for_this_ones() {
    let (desk, mine, theirs) = (desk(), Scratch::new(), Scratch::new());
    desk.room.hold(1);
    let (me, them) = (
        caller(&desk, mine.path(), None),
        caller(&desk, theirs.path(), None),
    );
    let ends = json!({"action": "open", "kind": "terminal", "command": "sh -c 'exit 0'"});
    let call = crate::desk::Call {
        person: true,
        ..them.call()
    };
    let group = crate::projects::key(them.project.id);
    desk.person_terminal(&call, &group, "sh -c 'exit 0'")
        .unwrap();
    let session = crate::lock(&desk.sessions)
        .values()
        .next()
        .cloned()
        .unwrap();
    let until = session.wait(Some("never shown"), 0, std::time::Duration::from_secs(10));
    assert_eq!(until, crate::terminal::Until::Exited);
    let refused = desk.window(&me.call(), &ends).unwrap();
    assert!(
        refused.starts_with("not opened: 1 terminal is open"),
        "{refused}"
    );
    assert_eq!(crate::lock(&desk.sessions).len(), 1);
    desk.shutdown();
}
