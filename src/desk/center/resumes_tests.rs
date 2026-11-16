//! Tests of `resumes`: an agent that did not go on in its terminal as the app opened is one
//! quiet line there, with what the person can do about it.

use serde_json::json;

use super::super::Kind;
use super::super::tests::shown;
use crate::desk::restore::{Resume, Stage};
use crate::desk::tests::{caller, desk};
use crate::testing::Scratch;

#[test]
fn an_agent_that_did_not_go_on_is_one_quiet_line_on_its_terminal_with_its_way_on() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 20"});
    desk.window(&mine.call(), &open).unwrap();
    let resume = |stage| Resume {
        row: 1,
        name: "Claude Code",
        line: "claude --continue".into(),
        session: None,
        stage,
        tries: 0,
    };
    crate::lock(&desk.past)
        .resumes
        .insert("t1".into(), resume(Stage::Waiting));
    desk.feed.record(
        &crate::feed::Entity::Terminal { id: 1 },
        crate::feed::Op::Upserted,
    );
    assert!(shown(&desk).is_empty(), "one on its way is no news");
    for (stage, detail, button) in [
        (Stage::Failed, "Did not resume", "Retry"),
        (Stage::Continue, "Was open here", "Continue"),
    ] {
        crate::lock(&desk.past)
            .resumes
            .insert("t1".into(), resume(stage));
        desk.feed.record(
            &crate::feed::Entity::Terminal { id: 1 },
            crate::feed::Op::Upserted,
        );
        let events = shown(&desk);
        assert_eq!(events.len(), 1, "{events:?}");
        let event = &events[0];
        assert_eq!((event.kind, event.tab.as_str()), (Kind::Resume, "t1"));
        assert_eq!(
            (event.title.as_str(), event.detail.as_str()),
            ("Claude Code", detail)
        );
        assert!(!event.look.urgent, "it is said quietly");
        let labels: Vec<&str> = event.buttons.iter().map(|button| button.label).collect();
        // A quiet line can be put away, as a program that finished.
        assert_eq!(labels, [button, "Show", "Dismiss"]);
    }
}
