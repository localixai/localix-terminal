//! Tests of `hub`.

use std::io::BufReader;
use std::os::unix::net::UnixStream;

use serde_json::json;

use super::*;
use crate::wire::{Next, read_frame};

/// A subscriber: its end of the socket to read notifications from, and its outbox in the hub.
fn subscriber(hub: &Hub, conn: u64, owner: &str, events: &[&str]) -> BufReader<UnixStream> {
    let (server, client) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let out = Outbox::spawn(server, "test-write").unwrap();
    let events: Vec<String> = events.iter().map(ToString::to_string).collect();
    hub.subscribe(conn, out, owner.to_string(), &events);
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
fn an_event_reaches_the_subscribers_of_its_owner_only() {
    let hub = Hub::default();
    let mut mine = subscriber(&hub, 1, "me", &["comment/added", "app/quitting"]);
    let mut other = subscriber(&hub, 3, "you", &["comment/added", "app/quitting"]);
    let mut not_asked = subscriber(&hub, 4, "me", &["tab/closed"]);
    hub.publish("comment/added", Some("me"), json!({"text": "hi"}));
    assert_eq!(next(&mut mine).as_deref(), Some("comment/added"));
    assert_eq!(next(&mut other), None);
    assert_eq!(next(&mut not_asked), None);
    // An event of no owner (the app quitting) reaches every subscriber to it.
    hub.publish("app/quitting", None, json!({}));
    assert_eq!(next(&mut mine).as_deref(), Some("app/quitting"));
    assert_eq!(next(&mut other).as_deref(), Some("app/quitting"));
    assert_eq!(next(&mut not_asked), None);
}

#[test]
fn a_step_is_told_to_the_connection_that_asked_and_a_gone_one_is_forgotten() {
    let hub = Hub::default();
    let mut a = subscriber(&hub, 1, "me", &["tab/closed"]);
    let mut b = subscriber(&hub, 2, "me", &["tab/closed"]);
    hub.tell(2, "tab/closed", json!({"text": "CLICK"}));
    assert_eq!(next(&mut b).as_deref(), Some("tab/closed"));
    assert_eq!(next(&mut a), None);
    assert_eq!(hub.count(), 2);
    hub.unsubscribe(1);
    assert_eq!(hub.count(), 1);
}
