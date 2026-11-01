//! Protocol v2 against a daemon of a test build (no window): the handshake, answers by id,
//! calls running at once, notifications by subscription, bytes after a response, a retried call
//! done once, and a request that panics. Every check goes over the socket, as a client does.

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use llx_desk::client::{Client, Error, Link};
use llx_desk::harness::{Home, asked};
use llx_desk::wire::code;
use serde_json::{Value, json};

fn home(name: &str) -> Home {
    Home::new(env!("CARGO_BIN_EXE_llx-desk"), name)
}

fn window(owner: &str, args: &Value) -> Value {
    json!({"owner": owner, "cwd": "/tmp", "args": args})
}

const OPEN_SLEEPER: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 60", "yield_ms": 300}"#;

#[test]
fn the_handshake_comes_first_and_says_protocol_build_and_capabilities() {
    let home = home("hello");
    home.start(&[("LLX_DESK_BUILD", "build-a")]);
    // Without `initialize` nothing is served.
    let raw = Link::open(
        UnixStream::connect(home.socket()).unwrap(),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    let Err(Error::Rpc(refused)) = raw.request("ping", json!({}), Some(Duration::from_secs(5)))
    else {
        panic!("served without a handshake");
    };
    assert_eq!(refused.code, code::NOT_INITIALIZED);
    // A client that speaks an older protocol is told so.
    let old = raw.request(
        "initialize",
        json!({"protocolVersion": 1}),
        Some(Duration::from_secs(5)),
    );
    assert!(matches!(old, Err(Error::Rpc(_))));

    let (client, _) = home.client();
    assert_eq!(client.server.build, "build-a");
    let hello = raw
        .request(
            "initialize",
            json!({"protocolVersion": 2, "capabilities": {"binary": true}}),
            None,
        )
        .unwrap()
        .value;
    assert_eq!(hello["protocolVersion"], 2);
    assert_eq!(hello["capabilities"]["notifications"][0], "comment/added");
    assert_eq!(
        hello["serverInfo"]["pid"],
        u64::try_from(home.pid().unwrap()).unwrap()
    );
    assert_eq!(asked(&client, "ping", json!({})), json!({}));
}

#[test]
fn a_line_that_is_no_message_is_answered_and_the_connection_goes_on() {
    use std::io::{BufRead, BufReader, Write};
    let home = home("garbage");
    home.start(&[]);
    let mut stream = UnixStream::connect(home.socket()).unwrap();
    // An older client's frame, then garbage, then a real request.
    stream
        .write_all(b"{\"op\":\"status\"}\nnot json\n")
        .unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n")
        .unwrap();
    let mut lines = BufReader::new(stream).lines();
    let first: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(first["error"]["code"], code::INVALID);
    let second: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(second["error"]["code"], code::PARSE);
    let third: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(
        (third["id"].clone(), third["error"]["code"].clone()),
        (json!(9), json!(code::NOT_INITIALIZED))
    );
}

#[test]
fn requests_are_pipelined_and_answered_by_id_while_others_run() {
    let home = home("pipeline");
    home.start(&[]);
    let (client, _) = home.client();
    let client = Arc::new(client);
    for tab in 1..=2 {
        let opened = asked(
            &client,
            "window/call",
            window("o", &serde_json::from_str(OPEN_SLEEPER).unwrap()),
        );
        assert!(
            opened["text"]
                .as_str()
                .unwrap()
                .contains(&format!("t{tab}"))
        );
    }
    // Two calls that each wait a second in a tab of their own, and a ping beside them.
    let wait = |tab: &str| {
        let client = Arc::clone(&client);
        let args =
            json!({"action": "interact", "tab": tab, "wait_for": "never shown", "yield_ms": 1000});
        std::thread::Builder::new()
            .name("test-call".into())
            .spawn(move || {
                let began = Instant::now();
                client.request("window/call", window("o", &args)).unwrap();
                began.elapsed()
            })
            .unwrap()
    };
    let began = Instant::now();
    let (a, b) = (wait("t1"), wait("t2"));
    let pinged = Instant::now();
    asked(&client, "ping", json!({}));
    assert!(
        pinged.elapsed() < Duration::from_millis(500),
        "a ping waited for the calls"
    );
    let (a, b) = (a.join().unwrap(), b.join().unwrap());
    assert!(
        a >= Duration::from_millis(900) && b >= Duration::from_millis(900),
        "{a:?} {b:?}"
    );
    // At once, not one after the other.
    assert!(
        began.elapsed() < Duration::from_millis(1800),
        "{:?}",
        began.elapsed()
    );
}

#[test]
fn a_comment_reaches_a_subscriber_and_is_told_again_until_it_is_acknowledged() {
    let home = home("notify");
    home.start(&[]);
    let (client, received) = home.client();
    asked(
        &client,
        "window/call",
        window("o", &serde_json::from_str(OPEN_SLEEPER).unwrap()),
    );
    let subscribed = asked(&client, "events/subscribe", json!({"owner": "o"}));
    assert!(subscribed["events"].as_array().unwrap().len() == 5);
    asked(
        &client,
        "test/say",
        json!({"tab": "t1", "words": "this button is wrong"}),
    );
    let (method, note) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(method, "comment/added");
    assert_eq!(
        (
            note["owner"].clone(),
            note["tab"].clone(),
            note["text"].clone()
        ),
        (json!("o"), json!("t1"), json!("this button is wrong"))
    );

    // A client that was away subscribes again and is told what was not acknowledged.
    let (again, received_again) = home.client();
    asked(&again, "events/subscribe", json!({"owner": "o"}));
    let (_, replay) = received_again.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(replay["id"], note["id"]);
    asked(
        &again,
        "comment/ack",
        json!({"owner": "o", "ids": [note["id"]]}),
    );
    let (third, received_third) = home.client();
    asked(&third, "events/subscribe", json!({"owner": "o"}));
    assert!(
        received_third
            .recv_timeout(Duration::from_millis(400))
            .is_err(),
        "told again after the acknowledgement"
    );

    // Another owner's subscriber hears nothing of it; closing a tab and stopping are told.
    let (other, heard_by_other) = home.client();
    asked(&other, "events/subscribe", json!({"owner": "someone else"}));
    asked(
        &client,
        "window/call",
        window("o", &json!({"action": "close", "tab": "t1"})),
    );
    let (method, closed) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        (method.as_str(), closed["tab"].clone()),
        ("tab/closed", json!("t1"))
    );
    asked(&client, "daemon/stop", json!({}));
    let (method, _) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(method, "daemon/stopping");
    let (method, _) = heard_by_other.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(method, "daemon/stopping");
}

#[test]
fn bytes_follow_a_response_on_the_same_connection() {
    let home = home("bytes");
    home.start(&[]);
    let (client, _) = home.client();
    let answer = client
        .request("test/blob", json!({"length": 300_000}))
        .unwrap();
    let blob = answer.blob.unwrap();
    assert_eq!(blob.mime, "image/jpeg");
    assert_eq!(blob.bytes.len(), 300_000);
    assert_eq!(blob.bytes[251], 0);
    assert_eq!(blob.bytes[252], 1);
    // The connection is in step after them.
    assert_eq!(asked(&client, "ping", json!({})), json!({}));
}

#[test]
fn a_call_asked_again_with_its_key_is_done_once_also_on_a_new_connection() {
    let home = home("again");
    home.start(&[]);
    let (first, _) = home.client();
    let counted = |client: &Client, key: &str| {
        asked(client, "test/count", json!({"idempotencyKey": key}))["count"].clone()
    };
    let one = counted(&first, "k1");
    assert_eq!(counted(&first, "k1"), one);
    let (second, _) = home.client();
    assert_eq!(counted(&second, "k1"), one);
    assert_ne!(counted(&second, "k2"), one);

    // A destructive call: the person's comment is taken once, and the retry gets the same words.
    asked(
        &first,
        "window/call",
        window("o", &serde_json::from_str(OPEN_SLEEPER).unwrap()),
    );
    asked(
        &first,
        "test/say",
        json!({"tab": "t1", "words": "look at the title"}),
    );
    let said = |client: &Client| {
        asked(
            client,
            "window/said",
            json!({"owner": "o", "idempotencyKey": "s1"}),
        )["text"]
            .clone()
    };
    let text = said(&first);
    assert!(text.as_str().unwrap().contains("look at the title"));
    assert_eq!(said(&second), text);
    let after = asked(&first, "window/said", json!({"owner": "o"}));
    assert_eq!(after["text"], "");
}

#[test]
fn a_request_that_panics_is_that_requests_error_and_the_daemon_goes_on() {
    let home = home("panic");
    home.start(&[]);
    let (client, _) = home.client();
    let Err(Error::Rpc(error)) = client.request("test/panic", json!({})) else {
        panic!("a panic was not an error");
    };
    assert_eq!(error.code, code::INTERNAL);
    assert!(error.message.contains("the daemon goes on"));
    assert_eq!(asked(&client, "ping", json!({})), json!({}));
    let Err(Error::Rpc(unknown)) = client.request("no/such", json!({})) else {
        panic!("an unknown method was served");
    };
    assert_eq!(unknown.code, code::NO_METHOD);
}
