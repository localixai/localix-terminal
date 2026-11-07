//! The API, once: every method the server answers, with what it is for, its params as JSON
//! Schema, and whether it is work (it acts in the window or a terminal and takes as long as it
//! takes, so it runs on a thread of its own). The dispatch (`methods::call`), `api/describe` and,
//! through it, every adapter (the CLI, `llx-desk mcp`, a plugin) read this one table: a method
//! added here is served, described and reachable everywhere, with nothing else to keep in step.

use serde_json::{Value, json};

use super::Shared;
use super::conn::Conn;
use super::hub::EVENTS;
use super::idem::Outcome;
use super::methods as m;
use crate::wire::PROTOCOL;

/// A method of the API.
pub(super) struct Method {
    pub name: &'static str,
    pub summary: &'static str,
    /// Its params, as JSON Schema.
    pub params: fn() -> Value,
    /// It acts in the window or a terminal: a thread of its own, so the connection still
    /// answers a ping meanwhile.
    pub work: bool,
    pub run: fn(&Shared, &Conn, &Value) -> Outcome,
}

const fn method(
    name: &'static str,
    summary: &'static str,
    params: fn() -> Value,
    work: bool,
    run: fn(&Shared, &Conn, &Value) -> Outcome,
) -> Method {
    Method {
        name,
        summary,
        params,
        work,
        run,
    }
}

/// Every method, in the order `api/describe` lists them.
pub(super) const METHODS: [Method; 11] = [
    method(
        "window/call",
        "One call of the window tool: open, interact, screenshot, close, show, list. `args` is the \
         tool's arguments (`window.schema`); a screenshot comes as bytes after the response when the \
         client said it takes them.",
        call_params,
        true,
        m::window_call,
    ),
    method(
        "window/said",
        "What the person sent from the window to the caller's tabs since it was last asked, taken.",
        caller_params,
        true,
        m::said,
    ),
    method(
        "window/end",
        "Close the caller's tabs.",
        caller_params,
        true,
        m::end,
    ),
    method(
        "window/present",
        "Bring the window to the person: with no tab, what the project had open (else a terminal in \
         `cwd`); with `go`, on the project.",
        present_params,
        true,
        m::present,
    ),
    method(
        "comment/ack",
        "The person's notes the caller received (`comment/added`), by id: not told again.",
        ack_params,
        false,
        m::acknowledge,
    ),
    method(
        "events/subscribe",
        "Be told of `events` (all when none) of the caller's tabs (the app quitting is everyone's), as \
         notifications on this connection; the person's unacknowledged notes are told again now.",
        subscribe_params,
        false,
        m::subscribe,
    ),
    method(
        "events/unsubscribe",
        "Be told nothing more on this connection.",
        no_params,
        false,
        m::unsubscribe,
    ),
    method(
        "api/describe",
        "This description: the methods, the events, and the window tool's own.",
        no_params,
        false,
        m::describe,
    ),
    method("ping", "An empty answer.", no_params, false, m::ping),
    method(
        "app/status",
        "What the app holds: its build, its connections, the window, every project's tabs.",
        no_params,
        false,
        m::status,
    ),
    method(
        "app/quit",
        "Quit the app (its tabs and terminals end), once this answer is written.",
        no_params,
        false,
        m::quit,
    ),
];

/// The method named `name`.
pub(super) fn find(name: &str) -> Option<&'static Method> {
    METHODS.iter().find(|method| method.name == name)
}

/// What `api/describe` answers: the protocol, this build, every method with its params, the
/// events, and the window tool (its description and its arguments' schema), so an adapter needs
/// nothing of its own to offer the tool.
pub(super) fn describe(build: &str) -> Value {
    let methods: Vec<Value> = (METHODS.iter())
        .map(|method| {
            json!({
                "name": method.name,
                "summary": method.summary,
                "params": (method.params)(),
            })
        })
        .collect();
    json!({
        "protocolVersion": PROTOCOL,
        "server": {"name": "llx-desk", "version": env!("CARGO_PKG_VERSION"), "build": build},
        "methods": methods,
        "events": EVENTS,
        "window": {
            "description": crate::desk::description(),
            "schema": crate::desk::schema(),
        },
    })
}

fn object(properties: &Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

fn string(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

const PROJECT: &str = "The project the caller works in: its folder, an absolute path. Its tabs, board and history are the project's.";
const AGENT: &str =
    "The caller's name within the project (a worker's); none: the project's own agent.";

fn no_params() -> Value {
    object(&json!({}), &[])
}

fn caller_params() -> Value {
    object(
        &json!({"project": string(PROJECT), "agent": string(AGENT)}),
        &["project"],
    )
}

fn environment() -> Value {
    json!({
        "type": "object",
        "additionalProperties": {"type": "string"},
        "description": "The caller's environment, for a program a terminal starts.",
    })
}

fn call_params() -> Value {
    object(
        &json!({
            "project": string(PROJECT),
            "agent": string(AGENT),
            "cwd": string("The caller's directory: relative paths start here."),
            "env": environment(),
            "args": {"type": "object", "description": "The window tool's arguments: `window.schema` of this description."},
            "peek": {"type": "boolean", "description": "A look for a pane: the person's notes stay for the agent."},
            "idempotencyKey": string("The same key again is answered once, from the first call."),
        }),
        &["project", "cwd", "args"],
    )
}

fn present_params() -> Value {
    object(
        &json!({
            "project": string(PROJECT),
            "cwd": string("Where a terminal starts when the window has no tab."),
            "env": environment(),
            "go": {"type": "boolean", "description": "The window goes to the project (`llx-desk open DIR`)."},
        }),
        &["project", "cwd"],
    )
}

fn ack_params() -> Value {
    object(
        &json!({
            "project": string(PROJECT),
            "agent": string(AGENT),
            "ids": {"type": "array", "items": {"type": "integer"}, "description": "The notes' ids."},
        }),
        &["project", "ids"],
    )
}

fn subscribe_params() -> Value {
    object(
        &json!({
            "project": string("The caller's project: only its events (with `agent`, only that agent's)."),
            "agent": string(AGENT),
            "events": {"type": "array", "items": {"type": "string", "enum": EVENTS}},
        }),
        &["project"],
    )
}

#[cfg(test)]
mod tests {
    use super::{METHODS, describe, find};

    #[test]
    fn every_method_is_named_once_and_described_with_its_params() {
        for (at, method) in METHODS.iter().enumerate() {
            assert!(
                METHODS
                    .iter()
                    .skip(at + 1)
                    .all(|other| other.name != method.name),
                "{} twice",
                method.name
            );
            assert_eq!((method.params)()["type"], "object", "{}", method.name);
            assert!(find(method.name).is_some());
        }
        let described = describe("b");
        assert_eq!(
            described["methods"].as_array().unwrap().len(),
            METHODS.len()
        );
        assert!(described["window"]["schema"]["properties"]["action"].is_object());
        assert_eq!(find("window/call").map(|m| m.work), Some(true));
        assert!(find("nothing/here").is_none());
    }
}
