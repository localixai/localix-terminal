//! The methods of the protocol: what each request asks of the desk, and the shape of its answer.
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
use crate::projects::Project;
use crate::wire::{Blob, PROTOCOL, RpcError, code};

/// What a request asks, done: the method the API names (`api::METHODS`), else a test build's
/// own (`probes`), else an error that names it.
pub(super) fn call(shared: &Shared, conn: &Conn, method: &str, params: &Value) -> Outcome {
    if let Some(known) = super::api::find(method) {
        return (known.run)(shared, conn, params);
    }
    probe(shared, method, params).unwrap_or_else(|| {
        Outcome::failed(RpcError::new(
            code::NO_METHOD,
            format!("no method `{method}`: `api/describe` lists them"),
        ))
    })
}

/// An answer that is a value or an error, with no bytes after it.
fn done(result: Result<Value, RpcError>) -> Outcome {
    Outcome { result, blob: None }
}

pub(super) fn ping(_shared: &Shared, _conn: &Conn, _params: &Value) -> Outcome {
    done(Ok(json!({})))
}

pub(super) fn describe(shared: &Shared, _conn: &Conn, _params: &Value) -> Outcome {
    done(Ok(super::api::describe(&shared.build)))
}

pub(super) fn end(shared: &Shared, _conn: &Conn, params: &Value) -> Outcome {
    done(who(shared, params).map(|who| {
        let closed = shared.desk.end(&who.owner);
        let plural = if closed == 1 { "" } else { "s" };
        json!({"text": format!("Closed {closed} tab{plural}.")})
    }))
}

pub(super) fn said(shared: &Shared, _conn: &Conn, params: &Value) -> Outcome {
    done(
        who(shared, params)
            .map(|who| json!({"text": shared.desk.said(&who.owner).unwrap_or_default()})),
    )
}

pub(super) fn acknowledge(shared: &Shared, _conn: &Conn, params: &Value) -> Outcome {
    done(who(shared, params).map(|who| {
        let ids: Vec<u64> = (params
            .get("ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten())
        .filter_map(Value::as_u64)
        .collect();
        shared.desk.acknowledge(&who.owner, &ids);
        json!({})
    }))
}

pub(super) fn unsubscribe(shared: &Shared, conn: &Conn, _params: &Value) -> Outcome {
    shared.hub.unsubscribe(conn.id);
    done(Ok(json!({})))
}

pub(super) fn status(shared: &Shared, _conn: &Conn, _params: &Value) -> Outcome {
    done(Ok(status_of(shared)))
}

/// The caller ends the server once the answer is written (`conn::take`).
pub(super) fn quit(_shared: &Shared, _conn: &Conn, _params: &Value) -> Outcome {
    done(Ok(json!({"text": "quitting"})))
}

pub(super) fn subscribe(shared: &Shared, conn: &Conn, params: &Value) -> Outcome {
    done(subscribed(shared, conn, params))
}

pub(super) fn present(shared: &Shared, _conn: &Conn, params: &Value) -> Outcome {
    done(presented(shared, params))
}

/// The methods only a test build has.
#[cfg(feature = "test-server")]
fn probe(shared: &Shared, method: &str, params: &Value) -> Option<Outcome> {
    super::probes::call(shared, method, params)
}

#[cfg(not(feature = "test-server"))]
fn probe(_shared: &Shared, _method: &str, _params: &Value) -> Option<Outcome> {
    None
}

/// Who calls: an agent (`agent`, none for the project's own) of the project at `project`, found
/// or made.
struct Who {
    owner: String,
    project: Project,
    agent: Option<String>,
}

fn who(shared: &Shared, params: &Value) -> Result<Who, RpcError> {
    let path = PathBuf::from(text(params, "project")?);
    let project =
        (shared.desk.project(&path)).map_err(|why| RpcError::new(code::BAD_PARAMS, why))?;
    let agent = (params.get("agent").and_then(Value::as_str))
        .filter(|agent| !agent.is_empty())
        .map(str::to_string);
    Ok(Who {
        owner: crate::projects::owner(&project, agent.as_deref()),
        project,
        agent,
    })
}

fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(code::BAD_PARAMS, format!("`{name}` is a string")))
}

/// Subscribe the connection to `events` of a caller's (`project`, `agent`; or of everyone's),
/// and tell it again the notes the person sent that nobody acknowledged: a break loses none.
fn subscribed(shared: &Shared, conn: &Conn, params: &Value) -> Result<Value, RpcError> {
    // A subscriber is always someone's: no connection hears every project's comments.
    let owner = who(shared, params)?.owner;
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
    if known.iter().any(|e| e == "comment/added") {
        for said in shared.desk.pending(&owner) {
            shared
                .hub
                .tell(conn.id, "comment/added", shared.desk.note(&owner, &said));
        }
    }
    Ok(json!({"events": known}))
}

fn status_of(shared: &Shared) -> Value {
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
pub(super) fn window_call(shared: &Shared, conn: &Conn, params: &Value) -> Outcome {
    let run = || -> Result<(Value, Option<Blob>), RpcError> {
        let who = who(shared, params)?;
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
        let call = Call {
            owner: &who.owner,
            project: &who.project,
            agent: who.agent.as_deref(),
            cwd: &cwd,
            vars: vars.as_deref(),
            peek: params.get("peek").and_then(Value::as_bool).unwrap_or(false),
            again: None,
            person: false,
            window: None,
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

/// The person opened the app: the window to them (`Desk::present`), a terminal in it when it has
/// no tab, started in `cwd` with the caller's environment.
fn presented(shared: &Shared, params: &Value) -> Result<Value, RpcError> {
    let who = who(shared, params)?;
    let cwd = PathBuf::from(text(params, "cwd")?);
    let vars = environment(params);
    let call = Call {
        owner: &who.owner,
        project: &who.project,
        agent: who.agent.as_deref(),
        cwd: &cwd,
        vars: vars.as_deref(),
        peek: true,
        again: None,
        person: false,
        window: None,
    };
    let go = params.get("go").and_then(Value::as_bool).unwrap_or(false);
    (shared.desk.present(&call, go))
        .map(|said| json!({"text": said}))
        .map_err(|why| RpcError::new(code::FAILED, why))
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
