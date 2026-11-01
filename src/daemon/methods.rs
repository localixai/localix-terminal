//! The methods of protocol v2: what each request asks of the desk, and the shape of its answer.
//! Every result is an object; a failed call is an error with the code that says whether trying
//! again could help. Methods run on a connection's worker, one request at a time each.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::Shared;
use super::conn::Conn;
use super::hub::EVENTS;
use super::idem::Outcome;
use crate::desk::{Call, Failure};
use crate::wire::{Blob, PROTOCOL, RpcError, code};

/// Methods that act in the window or a terminal, which take as long as they take: each runs on
/// a thread of its own, so the connection still answers a ping.
pub(super) fn is_work(method: &str) -> bool {
    method.starts_with("window/")
}

/// What a request asks, done; `id` is the request's, which a goal's steps name.
pub(super) fn call(
    shared: &Shared,
    conn: &Conn,
    id: &Value,
    method: &str,
    params: &Value,
) -> Outcome {
    let result = match method {
        "ping" => Ok(json!({})),
        "window/call" => return window_call(shared, conn, id, params),
        "window/end" => owner(params).map(|owner| {
            let closed = shared.desk.end(owner);
            let plural = if closed == 1 { "" } else { "s" };
            json!({"text": format!("Closed {closed} tab{plural}.")})
        }),
        "window/said" => {
            owner(params).map(|owner| json!({"text": shared.desk.said(owner).unwrap_or_default()}))
        }
        "comment/ack" => owner(params).map(|owner| {
            let ids: Vec<u64> = (params
                .get("ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .filter_map(Value::as_u64)
            .collect();
            shared.desk.acknowledge(owner, &ids);
            json!({})
        }),
        "events/subscribe" => Ok(subscribe(shared, conn, params)),
        "events/unsubscribe" => {
            shared.hub.unsubscribe(conn.id);
            Ok(json!({}))
        }
        "daemon/status" => Ok(status(shared)),
        // The caller ends it once the answer is written (`conn::take`).
        "daemon/stop" => Ok(json!({"text": "stopping"})),
        other => match probe(shared, other, params) {
            Some(outcome) => return outcome,
            None => Err(RpcError::new(
                code::NO_METHOD,
                format!("no method `{other}`"),
            )),
        },
    };
    Outcome { result, blob: None }
}

/// The methods only a test build has.
#[cfg(feature = "test-daemon")]
fn probe(shared: &Shared, method: &str, params: &Value) -> Option<Outcome> {
    super::probes::call(shared, method, params)
}

#[cfg(not(feature = "test-daemon"))]
fn probe(_shared: &Shared, _method: &str, _params: &Value) -> Option<Outcome> {
    None
}

fn owner(params: &Value) -> Result<&str, RpcError> {
    text(params, "owner")
}

fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(code::BAD_PARAMS, format!("`{name}` is a string")))
}

/// Subscribe the connection to `events` of `owner`'s (or of every owner's), and tell it again
/// the notes the person sent that nobody acknowledged: a break loses none.
fn subscribe(shared: &Shared, conn: &Conn, params: &Value) -> Value {
    let owner = params
        .get("owner")
        .and_then(Value::as_str)
        .map(String::from);
    let wanted: Vec<String> = match params.get("events").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        None => EVENTS.iter().map(ToString::to_string).collect(),
    };
    let known: Vec<String> = wanted
        .into_iter()
        .filter(|e| EVENTS.contains(&e.as_str()))
        .collect();
    shared
        .hub
        .subscribe(conn.id, conn.out.clone(), owner.clone(), &known);
    if let Some(owner) = owner.filter(|_| known.iter().any(|e| e == "comment/added")) {
        for said in shared.desk.pending(&owner) {
            shared.hub.tell(conn.id, "comment/added", said.note(&owner));
        }
    }
    json!({"events": known})
}

fn status(shared: &Shared) -> Value {
    let text = format!(
        "llx-desk {} (pid {})\nbuild: {}, protocol {PROTOCOL}\nconnections: {}, subscribed: {}\n{}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        shared.build,
        shared.connections.load(Ordering::SeqCst),
        shared.hub.count(),
        shared.desk.status()
    );
    json!({"text": text, "pid": std::process::id(), "build": shared.build, "protocol": PROTOCOL})
}

/// One call of the `window` tool.
fn window_call(shared: &Shared, conn: &Conn, id: &Value, params: &Value) -> Outcome {
    let run = || -> Result<(Value, Option<Blob>), RpcError> {
        let owner = owner(params)?;
        let cwd = PathBuf::from(text(params, "cwd")?);
        let args = params
            .get("args")
            .filter(|args| args.is_object())
            .ok_or_else(|| {
                RpcError::new(
                    code::BAD_PARAMS,
                    "`args` is the window's arguments as an object",
                )
            })?;
        let vars = environment(params);
        let progress = |tab: &str, step: &str| {
            // The key names the call to a client that made it with one.
            let told =
                json!({"id": id, "key": params.get("idempotencyKey"), "tab": tab, "text": step});
            shared.hub.tell(conn.id, "goal/step", told);
        };
        let call = Call {
            owner,
            cwd: &cwd,
            vars: vars.as_deref(),
            key: params.get("key").and_then(Value::as_str),
            progress: Some(&progress),
        };
        let answered = shared
            .desk
            .call(&call, args)
            .map_err(|failure| match failure {
                Failure::Failed(why) => RpcError::new(code::FAILED, why),
                Failure::Fatal(why) => RpcError::new(code::FATAL, why),
            })?;
        let Some(shot) = answered.shot.as_ref() else {
            return Ok((json!({"text": answered.text_with(None)}), None));
        };
        if conn.carries_bytes() {
            let blob = Blob {
                mime: "image/jpeg".into(),
                bytes: shot.jpeg.clone(),
            };
            let result = json!({"text": answered.text_with(None), "shot": {"width": shot.width, "height": shot.height}});
            return Ok((result, Some(blob)));
        }
        let saved = crate::shots::save(&crate::paths::shots(), &shot.jpeg)
            .map_err(|why| RpcError::new(code::FAILED, why))?;
        let line = format!(
            "Screenshot saved ({}x{}): {}\nRead that file to see it.",
            shot.width,
            shot.height,
            saved.display()
        );
        Ok((json!({"text": answered.text_with(Some(&line))}), None))
    };
    match run() {
        Ok((result, blob)) => Outcome {
            result: Ok(result),
            blob,
        },
        Err(error) => Outcome::failed(error),
    }
}

/// The caller's environment, sent as an object, for a program started in a terminal.
fn environment(params: &Value) -> Option<Vec<(String, String)>> {
    let vars = params.get("env")?.as_object()?;
    Some(
        vars.iter()
            .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
            .collect(),
    )
}
