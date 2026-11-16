//! Tests of `refused`: a terminal the app had no room for is one quiet line of its project,
//! gone once a terminal opens there again.

use serde_json::json;

use super::super::Kind;
use super::super::tests::shown;
use crate::desk::tests::{caller, desk};
use crate::testing::Scratch;

#[test]
fn a_terminal_not_opened_for_want_of_room_is_one_quiet_line_until_one_opens() {
    let (desk, scratch) = (desk(), Scratch::new());
    desk.room.hold(1);
    let mine = caller(&desk, scratch.path(), None);
    let open = |command: &str| {
        let open =
            json!({"action": "open", "kind": "terminal", "command": command, "yield_ms": 200});
        desk.window(&mine.call(), &open)
    };
    assert!(!open("sleep 20").unwrap().starts_with("not opened"));
    assert!(open("sleep 21").unwrap().starts_with("not opened: "));
    assert!(open("sleep 22").unwrap().starts_with("not opened: "));
    let events = shown(&desk);
    assert_eq!(
        events.len(),
        1,
        "the latest refusal stands for the others: {events:?}"
    );
    let event = &events[0];
    assert_eq!((event.kind, event.tab.as_str()), (Kind::Refused, ""));
    assert_eq!(
        (event.title.as_str(), event.detail.as_str()),
        (
            "sleep 22",
            "Not opened: 1 terminal is open, the most this machine holds"
        )
    );
    assert_eq!(event.group, crate::projects::key(mine.project.id));
    assert!(!event.look.urgent, "it is said quietly");
    let labels: Vec<&str> = event.buttons.iter().map(|button| button.label).collect();
    assert_eq!(
        labels,
        ["Dismiss"],
        "it leads nowhere: there is nothing to show"
    );

    desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(!open("sleep 23").unwrap().starts_with("not opened"));
    assert!(
        shown(&desk).is_empty(),
        "a terminal opened there: it is no news"
    );
    desk.shutdown();
}
