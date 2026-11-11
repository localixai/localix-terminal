//! Tests of `leads`: an event leads to what it is about, opened where it has no tab.

use serde_json::json;

use super::super::event::{Do, Role};
use super::super::tests::shown;
use super::Leads;
use crate::desk::Call;
use crate::desk::tests::{caller, desk};
use crate::testing::Scratch;

#[test]
fn a_session_waiting_outside_the_window_leads_to_itself_continued_in_a_terminal() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    // Its tool was started in a folder of the project; the agent itself works in another
    // place, which is no folder of the project.
    let elsewhere = Scratch::new();
    let me = Call {
        runs: Some(("claude", elsewhere.path())),
        ..mine.call()
    };
    let ask = |words: &str| {
        let args = json!({"action": "interact", "ask": words});
        desk.window(&me, &args).unwrap();
    };
    ask("Claude asks: which branch?");

    let events = shown(&desk);
    assert_eq!(events.len(), 1, "{events:?}");
    let event = events[0].clone();
    // The row's own click is the one that leads there, as a tab's Show is; to the agent's own
    // folder, where it keeps the session.
    assert_eq!(event.labels(), ["Continue Here", "Dismiss"]);
    assert_eq!(event.buttons[0].role, Role::Open);
    assert_eq!(
        event.buttons[0].does,
        Do::Open(Leads::Session {
            owner: me.owner.to_string(),
            group: event.group.clone(),
            program: "claude".into(),
            folder: elsewhere.path().to_path_buf(),
        })
    );
    // A click that leads nowhere (no terminal opens in a folder that is not the project's)
    // leaves the line: the session still waits.
    desk.center_do(event.n, 0);
    assert_eq!(shown(&desk).len(), 1);
    // Continued in a terminal that opened, it is a tab from here on: its line is done with,
    // until the session asks again.
    desk.continued_here(me.owner);
    assert!(shown(&desk).is_empty());
    ask("Claude asks: and now?");
    assert_eq!(shown(&desk).len(), 1);
    desk.shutdown();
}
