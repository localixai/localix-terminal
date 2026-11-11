//! Tests of `mcp`: what a host is answered in each era, line by line, what the server says of
//! its one tool, and the shape of a failed tool call. No line here reaches the app.

use std::sync::Arc;

use serde_json::{Value, json};

use super::running::Running;
use super::wire::Out;
use super::{Server, failed, named};
use crate::client::Caller;
use crate::testing::Sink;

/// What the server writes when a host sends it `lines`.
fn served(lines: &[Value]) -> Vec<Value> {
    let sink = Sink::default();
    let (out, writer) = Out::spawn(sink.clone()).unwrap();
    let server = Server {
        out,
        caller: Caller::of(std::path::Path::new("/tmp"), None),
        cwd: "/tmp".into(),
        running: Arc::new(Running::new(1)),
    };
    for line in lines {
        server.take(&line.to_string());
    }
    drop(server);
    writer.join().unwrap();
    sink.lines()
}

/// A modern request: `method` under `id`, in revision `revision`, with more `params`.
fn modern(id: u64, method: &str, revision: &str, mut params: Value) -> Value {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": revision,
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

#[test]
fn a_legacy_host_opens_with_initialize_and_is_answered_its_ping() {
    let said = served(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
    ]);
    assert_eq!(said.len(), 3, "a notification is never answered");
    assert_eq!(said[0]["result"]["protocolVersion"], "2025-06-18");
    // A legacy result has no type: hosts of those revisions take none.
    assert_eq!(said[1], json!({"jsonrpc": "2.0", "id": 2, "result": {}}));
    assert_eq!(said[2]["error"]["code"], -32601);
}

#[test]
fn a_modern_host_discovers_the_server_and_is_served_with_no_handshake() {
    let said = served(&[
        modern(1, "server/discover", "2026-07-28", json!({})),
        // The handshake and the ping are gone from this revision.
        modern(2, "ping", "2026-07-28", json!({})),
        modern(3, "initialize", "2026-07-28", json!({})),
        modern(4, "tools/call", "2026-07-28", json!({"name": "other"})),
    ]);
    assert_eq!(said[0]["result"]["resultType"], "complete");
    assert_eq!(said[0]["result"]["supportedVersions"][0], "2026-07-28");
    assert_eq!(said[1]["error"]["code"], -32601);
    assert_eq!(said[2]["error"]["code"], -32601);
    assert_eq!(said[3]["error"]["code"], -32602);
}

#[test]
fn a_host_probing_for_the_era_is_told_which_revisions_to_try() {
    let said = served(&[
        modern(1, "server/discover", "2027-01-01", json!({})),
        // Discovery that names no revision is malformed, not a method unknown.
        json!({"jsonrpc": "2.0", "id": 2, "method": "server/discover"}),
    ]);
    assert_eq!(said[0]["error"]["code"], -32022);
    assert_eq!(said[0]["error"]["data"]["requested"], "2027-01-01");
    assert!(
        said[0]["error"]["data"]["supported"]
            .as_array()
            .unwrap()
            .len()
            > 1
    );
    assert_eq!(said[1]["error"]["code"], -32602);
}

#[test]
fn the_one_tool_is_the_whole_list_so_no_cursor_names_a_page() {
    let said = served(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {"cursor": "next"}}),
        modern(2, "tools/list", "2026-07-28", json!({"cursor": "next"})),
    ]);
    assert_eq!(said[0]["error"]["code"], -32602);
    assert_eq!(said[1]["error"]["code"], -32602);
}

#[test]
fn arguments_that_are_no_object_are_refused_and_a_failed_call_is_a_result_the_model_reads() {
    let args = json!({"name": "window", "arguments": 3});
    let said = served(&[modern(1, "tools/call", "2026-07-28", args)]);
    assert_eq!(said[0]["error"]["code"], -32602, "arguments are an object");
    assert_eq!(failed("no")["isError"], true);
}

#[test]
fn the_tool_says_truthfully_what_a_call_may_do() {
    let tool = named(&json!({"description": "d", "schema": {"type": "object"}}));
    assert_eq!(tool["name"], "window");
    assert!(tool["title"].is_string());
    // It opens the web, runs programs and removes cards: never read-only, never safe to repeat.
    let hints = &tool["annotations"];
    assert_eq!(hints["readOnlyHint"], false);
    assert_eq!(hints["idempotentHint"], false);
    assert_eq!(hints["destructiveHint"], true);
    assert_eq!(hints["openWorldHint"], true);
}
