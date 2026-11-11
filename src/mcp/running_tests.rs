//! Tests of the calls an MCP server runs: its share of them, one the host cancelled, and one
//! it follows.

use std::sync::Arc;

use serde_json::json;

use super::{Running, followed, progress};
use crate::mcp::wire::Out;
use crate::testing::Sink;

#[test]
fn a_call_the_host_cancelled_is_not_answered_and_frees_its_place() {
    let running = Running::new(1);
    assert!(running.start(&json!(7)));
    assert!(!running.start(&json!(8)), "one call at once here");
    running.cancel(&json!(7));
    assert!(
        !running.finish(&json!(7)),
        "a cancelled call is not answered"
    );
    assert!(running.start(&json!(8)));
    assert!(running.finish(&json!(8)));
}

#[test]
fn a_cancellation_of_no_running_call_changes_nothing() {
    let running = Running::new(2);
    running.cancel(&json!("late"));
    assert!(running.start(&json!("late")));
    assert!(running.finish(&json!("late")));
    // The ids 1 and "1" are two requests.
    assert!(running.start(&json!(1)));
    running.cancel(&json!("1"));
    assert!(running.finish(&json!(1)));
}

#[test]
fn only_a_call_the_host_follows_says_that_it_still_runs() {
    let sink = Sink::default();
    let (out, writer) = Out::spawn(sink.clone()).unwrap();
    let running = Arc::new(Running::new(4));
    let id = json!(1);
    // No token, or one that is neither a string nor a number: not followed.
    assert!(followed(&out, &running, &id, &json!(null)).is_none());
    assert!(followed(&out, &running, &id, &json!({"progressToken": {}})).is_none());
    assert!(followed(&out, &running, &id, &json!({"progressToken": "t"})).is_some());
    let said = progress(&json!("t"), 2);
    assert_eq!(said["method"], "notifications/progress");
    assert_eq!(said["params"]["progressToken"], "t");
    assert_eq!(said["params"]["progress"], 2);
    assert!(said.get("id").is_none(), "a notification has no id");
    drop(out);
    writer.join().unwrap();
    assert!(
        sink.lines().is_empty(),
        "nothing is said before the first beat"
    );
}

#[test]
fn nothing_is_said_of_a_call_that_ended_or_was_given_up() {
    let running = Running::new(4);
    let said = std::sync::atomic::AtomicUsize::new(0);
    let say = || {
        said.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    };
    running.while_running(&json!(1), say);
    assert_eq!(
        said.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "not started"
    );
    running.start(&json!(1));
    running.while_running(&json!(1), say);
    assert_eq!(said.load(std::sync::atomic::Ordering::Relaxed), 1);
    running.cancel(&json!(1));
    running.while_running(&json!(1), say);
    assert_eq!(
        said.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "given up"
    );
    running.finish(&json!(1));
    running.while_running(&json!(1), say);
    assert_eq!(said.load(std::sync::atomic::Ordering::Relaxed), 1, "ended");
}
