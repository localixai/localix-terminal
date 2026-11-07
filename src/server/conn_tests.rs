//! Tests of `conn`.

use std::io::BufReader;
use std::os::unix::net::UnixStream;

use serde_json::json;

use super::*;

#[test]
fn a_refused_connection_is_told_why_in_a_line_a_client_reads_as_an_error() {
    let (app, client) = UnixStream::pair().unwrap();
    refuse(&app, "64 connections are open: try again");
    let Ok(Next::Got(frame)) = read_frame(&mut BufReader::new(client)) else {
        panic!("not a frame");
    };
    let Message::Response {
        id,
        outcome: Err(error),
    } = frame.message
    else {
        panic!("not an error response");
    };
    assert!(id.is_null());
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(error.message.contains("try again"));
}

#[test]
fn a_kept_answer_is_found_only_by_the_same_method_caller_and_key() {
    let params = json!({"project": "/p", "agent": "a"});
    let key = kept_key("window/call", &params, "k");
    assert_eq!(key, kept_key("window/call", &params, "k"));
    assert_ne!(key, kept_key("window/said", &params, "k"));
    assert_ne!(key, kept_key("window/call", &params, "other"));
    assert_ne!(
        key,
        kept_key("window/call", &json!({"project": "/q", "agent": "a"}), "k")
    );
    assert_ne!(key, kept_key("window/call", &json!({"project": "/p"}), "k"));
    // However long the caller's names, the key kept is as short as the client's own.
    let long = json!({"project": "x".repeat(1 << 20)});
    assert!(kept_key("window/call", &long, "k").len() < 40);
}
