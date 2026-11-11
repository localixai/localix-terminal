//! Tests of the wire: what a line is taken for, and an error with what it carries.

use serde_json::json;

use super::{Asked, Out, asked};
use crate::testing::Sink;

#[test]
fn a_line_is_a_request_a_notification_or_refused_as_the_protocol_says() {
    let request = asked(r#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#);
    assert!(matches!(request, Asked::Request { ref method, .. } if method == "ping"));
    let note = asked(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert!(
        matches!(note, Asked::Note { ref method, .. } if method == "notifications/initialized")
    );
    // An answer to nothing asked.
    assert!(matches!(
        asked(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#),
        Asked::Nothing
    ));
    assert!(matches!(asked("{not json"), Asked::Refused(-32700, _)));
    // Valid JSON is no parse error: a batch, a bare value, an id that is no string or number.
    assert!(matches!(
        asked(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#),
        Asked::Refused(-32600, _)
    ));
    assert!(matches!(asked("3"), Asked::Refused(-32600, _)));
    assert!(matches!(
        asked(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#),
        Asked::Refused(-32600, _)
    ));
    assert!(matches!(
        asked(r#"{"jsonrpc":"2.0","id":{},"method":"ping"}"#),
        Asked::Refused(-32600, _)
    ));
}

#[test]
fn an_error_carries_its_data_only_when_it_has_some() {
    let sink = Sink::default();
    let (out, writer) = Out::spawn(sink.clone()).unwrap();
    out.error(&json!(1), -32601, "no method `x`");
    out.refuse(
        &json!("b"),
        -32022,
        "Unsupported protocol version",
        Some(&json!({"requested": "x"})),
    );
    drop(out);
    writer.join().unwrap();
    let said = sink.lines();
    assert_eq!(
        said[0],
        json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "no method `x`"}})
    );
    assert_eq!(said[1]["id"], "b");
    assert_eq!(said[1]["error"]["data"]["requested"], "x");
}
