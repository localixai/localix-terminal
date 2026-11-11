//! Comments and close notifications, including the person's independently owned tabs.

use super::*;
use crate::browser::Heard;

/// Records what the desk says happened.
#[derive(Default)]
struct Told(Mutex<Vec<(String, Option<String>, Value)>>);

impl Events for Told {
    fn publish(&self, event: &str, owner: Option<&str>, params: Value) {
        crate::lock(&self.0).push((event.to_string(), owner.map(String::from), params));
    }
}

#[test]
fn what_the_person_sent_reaches_the_owner_of_the_tab_once() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    desk.window(
        &me,
        &json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300}),
    )
    .unwrap();
    crate::lock(&desk.registry).hear("t1", None, "this button is wrong", true);
    let said = desk.said(&mine.owner).unwrap();
    assert!(
        said.contains("this button is wrong") && said.contains("untrusted"),
        "{said}"
    );
    assert!(desk.said(&mine.owner).is_none());
    desk.shutdown();
}

#[test]
fn a_comment_is_announced_kept_until_acknowledged_and_a_closed_tab_is_announced() {
    let told = Arc::new(Told::default());
    let events: Arc<dyn Events> = Arc::<Told>::clone(&told);
    let desk = Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        events,
        Arc::new(crate::store::Store::memory().unwrap()),
    );
    let scratch = Scratch::new();
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&me, &open).unwrap();

    desk.person_says("t1", "this button is wrong");
    let said = desk.pending(&mine.owner);
    assert_eq!(said.len(), 1);
    // The relay tells it from the feed, on a thread of its own.
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    {
        let told = crate::lock(&told.0);
        assert_eq!(told[0].0, "comment/added");
        assert_eq!(told[0].1.as_deref(), Some(mine.owner.as_str()));
        assert_eq!(told[0].2["agent"], "me");
        assert_eq!(
            told[0].2["project"],
            mine.project.path.display().to_string()
        );
        assert_eq!(told[0].2["text"], "this button is wrong");
        assert_eq!(told[0].2["id"], said[0].id);
    }
    // Told is not delivered: it stays until a call carries it, or it is acknowledged.
    assert_eq!(desk.pending(&mine.owner).len(), 1);
    desk.acknowledge(&mine.owner, &[said[0].id]);
    assert!(desk.pending(&mine.owner).is_empty());
    assert!(desk.said(&mine.owner).is_none());

    desk.window(&me, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    let told = crate::lock(&told.0);
    assert!(
        told.iter()
            .any(|(event, owner, params)| event == "tab/closed"
                && owner.as_deref() == Some(mine.owner.as_str())
                && params["tab"] == "t1")
    );
    drop(told);
    desk.shutdown();
}

#[test]
fn closing_a_tab_the_person_opened_is_news_to_no_agent() {
    let scratch = Scratch::new();
    let desk = desk();
    let mine = caller(&desk, scratch.path(), None);
    let open: Value = serde_json::from_str(
        r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#,
    )
    .unwrap();
    // The agent's own terminal, then one the person asked for in the window.
    desk.window(&mine.call(), &open).unwrap();
    let theirs = Call {
        person: true,
        window: None,
        ..mine.call()
    };
    desk.window(&theirs, &open).unwrap();
    desk.heard("t2", Heard::Closed("closed"));
    assert_eq!(desk.said(&mine.owner), None);
    desk.heard("t1", Heard::Closed("closed"));
    assert!(
        desk.said(&mine.owner)
            .is_some_and(|said| said.contains("closed"))
    );
    desk.shutdown();
}

#[test]
fn a_tab_the_person_opened_and_closed_is_told_to_no_subscriber() {
    let told = Arc::new(Told::default());
    let events: Arc<dyn Events> = Arc::<Told>::clone(&told);
    let desk = Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        events,
        Arc::new(crate::store::Store::memory().unwrap()),
    );
    let scratch = Scratch::new();
    let mine = caller(&desk, scratch.path(), None);
    let theirs = Call {
        person: true,
        ..mine.call()
    };
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&theirs, &open).unwrap();
    desk.window(&theirs, &open).unwrap();
    desk.heard("t1", Heard::Closed("closed"));
    desk.window(&mine.call(), &json!({"action": "close", "tab": "t2"}))
        .unwrap();
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    assert!(
        crate::lock(&told.0)
            .iter()
            .all(|(event, ..)| event != "tab/closed")
    );
    desk.shutdown();
}
