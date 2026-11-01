//! Tests of `hub`.

use std::io::BufReader;
use std::os::unix::net::UnixStream;

use serde_json::json;

use super::*;
use crate::wire::{Next, read_frame};

/// A subscriber: its end of the socket to read notifications from, and its outbox in the hub.
fn subscriber(hub: &Hub, conn: u64, owner: Option<&str>, events: &[&str]) -> BufReader<UnixStream> {
    let (daemon, client) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let out = Outbox::spawn(daemon, "test-write").unwrap();
    let events: Vec<String> = events.iter().map(ToString::to_string).collect();
    hub.subscribe(conn, out, owner.map(String::from), &events);
    BufReader::new(client)
}

fn next(input: &mut BufReader<UnixStream>) -> Option<String> {
    match read_frame(input) {
        Ok(Next::Got(frame)) => match frame.message {
            Message::Notification { method, .. } => Some(method),
            Message::Request { .. } | Message::Response { .. } => None,
        },
        Ok(Next::End | Next::Bad(_)) | Err(_) => None,
    }
}

use std::time::Duration;

#[test]
fn an_event_reaches_the_subscribers_of_its_owner_and_of_all_owners_only() {
    let hub = Hub::default();
    let mut mine = subscriber(&hub, 1, Some("me"), &["comment/added"]);
    let mut all = subscriber(&hub, 2, None, &["comment/added", "window/closed"]);
    let mut other = subscriber(&hub, 3, Some("you"), &["comment/added"]);
    let mut not_asked = subscriber(&hub, 4, Some("me"), &["tab/closed"]);
    hub.publish("comment/added", Some("me"), json!({"text": "hi"}));
    assert_eq!(next(&mut mine).as_deref(), Some("comment/added"));
    assert_eq!(next(&mut all).as_deref(), Some("comment/added"));
    assert_eq!(next(&mut other), None);
    assert_eq!(next(&mut not_asked), None);
    // An event of no owner reaches every subscriber to it.
    hub.publish("window/closed", None, json!({}));
    assert_eq!(next(&mut all).as_deref(), Some("window/closed"));
    assert_eq!(next(&mut mine), None);
}

#[test]
fn a_step_is_told_to_the_connection_that_asked_and_a_gone_one_is_forgotten() {
    let hub = Hub::default();
    let mut a = subscriber(&hub, 1, Some("me"), &["goal/step"]);
    let mut b = subscriber(&hub, 2, Some("me"), &["goal/step"]);
    hub.tell(2, "goal/step", json!({"text": "CLICK"}));
    assert_eq!(next(&mut b).as_deref(), Some("goal/step"));
    assert_eq!(next(&mut a), None);
    assert_eq!(hub.count(), 2);
    hub.unsubscribe(1);
    assert_eq!(hub.count(), 1);
}
