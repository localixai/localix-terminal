//! Tests of `wire`.

use super::*;

fn read(bytes: &[u8]) -> Next {
    read_frame(&mut &bytes[..]).unwrap()
}

#[test]
fn a_request_is_one_line_and_reads_back() {
    let sent = Message::request(7, "window/call", json!({"owner": "/p"}));
    let bytes = sent.encode(None);
    assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1);
    let Next::Got(Frame {
        message,
        blob: None,
    }) = read(&bytes)
    else {
        panic!("not a frame");
    };
    assert_eq!(message, sent);
}

#[test]
fn a_response_with_bytes_announces_them_and_the_bytes_follow_its_line() {
    let blob = Blob {
        mime: "image/jpeg".into(),
        bytes: vec![0, 10, 255, b'\n', 3],
    };
    let sent = Message::done(json!(1), json!({"text": "ok"}));
    let mut bytes = sent.encode(Some(&blob));
    // The next message follows the bytes: the reader takes exactly the announced length.
    bytes.extend(Message::notification("comment/added", json!({})).encode(None));
    let mut input = &bytes[..];
    let Next::Got(first) = read_frame(&mut input).unwrap() else {
        panic!("no first frame");
    };
    assert_eq!(first.blob, Some(blob));
    let Message::Response {
        outcome: Ok(result),
        ..
    } = first.message
    else {
        panic!("not a response");
    };
    assert_eq!(result["text"], "ok");
    let Next::Got(second) = read_frame(&mut input).unwrap() else {
        panic!("no second frame");
    };
    assert!(matches!(second.message, Message::Notification { .. }));
}

#[test]
fn an_error_response_has_a_code_and_a_message() {
    let sent = Message::failed(json!("a"), RpcError::new(code::FAILED, "no such tab"));
    let Next::Got(frame) = read(&sent.encode(None)) else {
        panic!("not a frame");
    };
    assert_eq!(frame.message, sent);
}

#[test]
fn an_invalid_request_id_is_not_treated_as_a_notification() {
    for id in [json!(true), json!([]), json!({})] {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": "window/call"});
        let mut bytes = serde_json::to_vec(&request).unwrap();
        bytes.push(b'\n');
        let Next::Bad(error) = read(&bytes) else {
            panic!("an invalid id was accepted: {request}");
        };
        assert_eq!(error.code, code::INVALID);
    }
}

#[test]
fn a_line_that_is_no_message_is_answered_and_the_next_still_reads() {
    let mut bytes = b"nope\n{\"jsonrpc\":\"2.0\"}\n{\"id\":1,\"method\":\"x\"}\n".to_vec();
    bytes.extend(Message::request(2, "ping", json!({})).encode(None));
    let mut input = &bytes[..];
    for want in [code::PARSE, code::INVALID, code::INVALID] {
        let Next::Bad(error) = read_frame(&mut input).unwrap() else {
            panic!("not refused");
        };
        assert_eq!(error.code, want);
    }
    assert!(matches!(read_frame(&mut input).unwrap(), Next::Got(_)));
    assert!(matches!(read_frame(&mut input).unwrap(), Next::End));
}

#[test]
fn announced_bytes_that_never_come_end_the_connection() {
    let blob = Blob {
        mime: "image/jpeg".into(),
        bytes: vec![1; 10],
    };
    let mut bytes = Message::done(json!(1), json!({})).encode(Some(&blob));
    bytes.truncate(bytes.len() - 4);
    assert!(read_frame(&mut &bytes[..]).is_err());
}

#[test]
fn a_notification_has_no_id_and_a_v1_frame_is_refused_with_a_reason() {
    let Next::Got(frame) =
        read(&Message::notification("tab/closed", json!({"tab": "w1"})).encode(None))
    else {
        panic!("not a frame");
    };
    assert!(matches!(frame.message, Message::Notification { .. }));
    let Next::Bad(error) = read(b"{\"ok\":false,\"text\":\"not a frame\"}\n") else {
        panic!("not refused");
    };
    assert!(
        error.message.contains("another protocol"),
        "{}",
        error.message
    );
}

#[test]
fn a_flushed_outbox_has_written_everything_queued_before_it() {
    use std::io::Read as _;
    use std::os::unix::net::UnixStream;
    let (near, mut far) = UnixStream::pair().unwrap();
    let out = pipe::Outbox::spawn(near, "outbox-test").unwrap();
    assert!(out.send(b"one\n".to_vec()) && out.send(b"two\n".to_vec()));
    out.flush();
    far.set_nonblocking(true).unwrap();
    let mut got = [0u8; 8];
    far.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"one\ntwo\n");
}

#[test]
fn an_error_without_a_request_to_name_keeps_its_null_id() {
    let sent = Message::failed(Value::Null, RpcError::new(code::UNAVAILABLE, "try again"));
    let Next::Got(frame) = read(&sent.encode(None)) else {
        panic!("not a frame");
    };
    assert_eq!(frame.message, sent);
    // A request or a result with no id is still no response.
    assert!(matches!(
        read(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"result\":{}}\n"),
        Next::Bad(_)
    ));
    let Next::Got(frame) = read(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"tab/closed\"}\n")
    else {
        panic!("not a frame");
    };
    assert!(matches!(frame.message, Message::Notification { .. }));
}

#[test]
fn a_line_over_the_limit_is_answered_and_the_next_still_reads() {
    let mut bytes = vec![b'x'; usize::try_from(MAX_LINE).unwrap() + 100];
    bytes.push(b'\n');
    bytes.extend(Message::request(2, "ping", json!({})).encode(None));
    let mut input = &bytes[..];
    let Next::Bad(error) = read_frame(&mut input).unwrap() else {
        panic!("not refused");
    };
    assert_eq!(error.code, code::INVALID);
    assert!(matches!(read_frame(&mut input).unwrap(), Next::Got(_)));
    assert!(matches!(read_frame(&mut input).unwrap(), Next::End));
}

#[test]
fn a_result_with_bytes_gets_its_binary_field_and_keeps_the_rest() {
    let blob = Blob {
        mime: "image/png".into(),
        bytes: vec![1, 2, 3],
    };
    let bytes =
        Message::done(json!(1), json!({"text": "ok", "binary": "stale"})).encode(Some(&blob));
    let Next::Got(frame) = read(&bytes) else {
        panic!("not a frame");
    };
    assert_eq!(frame.blob, Some(blob));
    let Message::Response {
        outcome: Ok(result),
        ..
    } = frame.message
    else {
        panic!("not a response");
    };
    assert_eq!(result["text"], "ok");
    assert_eq!(result["binary"]["length"], 3);
}

#[test]
fn the_peer_of_a_socket_pair_is_this_user() {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    assert_eq!(peer::peer_uid(&near), Some(peer::own_uid()));
    assert!(peer::is_own(&far));
}
