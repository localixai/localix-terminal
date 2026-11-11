//! `llx-desk mcp`: the app's API as an MCP server on stdio, for an agent whose host speaks MCP,
//! in the revision the host opens with (era.rs: with `initialize`, or request by request). It
//! holds nothing of its own: its one tool is the window tool as the app describes it
//! (`api/describe`; this build's own description while no app runs), and a call of it is
//! `window/call`, made as the CLI makes it (`client::ask`). Each call runs on a thread of its
//! own, so the host is answered meanwhile; one the host cancelled is not answered, and one it
//! follows is told that it still runs (running.rs). Only messages are written to stdout
//! (wire.rs).

mod era;
mod running;
mod wire;

use std::io::BufRead;
use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Value, json};

use crate::client::{Answer, Caller, Start, Trouble, ask, window_params};
use crate::out::warn;
use era::Era;
use running::Running;
use wire::{Asked, Out, asked};

/// Tool calls running at once.
const CALLS: usize = 16;
/// The one tool's name.
const TOOL: &str = "window";

/// Serve the host on this process's standard streams until its input ends; the exit code.
pub fn run(args: &[String]) -> u8 {
    let mut args = args.iter();
    let mut named = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--as" => {
                let Some(name) = args.next() else {
                    warn("llx-desk mcp: `--as` takes a name");
                    return 3;
                };
                named = Some(name.clone());
            }
            other => {
                warn(&format!("llx-desk mcp: no option `{other}`"));
                return 3;
            }
        }
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let caller = Caller::of(&cwd, named);
    let Ok((out, writer)) = Out::spawn(std::io::stdout()) else {
        warn("llx-desk mcp: no thread to write with");
        return 1;
    };
    let server = Server {
        out,
        caller,
        cwd: cwd.display().to_string(),
        running: Arc::new(Running::new(CALLS)),
    };
    // Bytes, not text: a line that is not UTF-8 is read lossily and answered like any other, and
    // does not end the host's session.
    let mut input = std::io::stdin().lock();
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if !matches!(input.read_until(b'\n', &mut raw), Ok(read) if read > 0) {
            break;
        }
        let line = String::from_utf8_lossy(&raw);
        if !line.trim().is_empty() {
            server.take(&line);
        }
    }
    // The host closed its side: the calls still running answer, and all is written, before
    // the process ends (the writer ends once the last of them lets go of its `Out`).
    drop(server);
    let _ = writer.join();
    0
}

struct Server {
    out: Out,
    caller: Caller,
    cwd: String,
    running: Arc<Running>,
}

impl Server {
    fn take(&self, line: &str) {
        match asked(line) {
            Asked::Request { id, method, params } => match era::of(&method, &params) {
                Ok(era) => self.answer(era, &id, &method, &params),
                Err(refusal) => {
                    (self.out).refuse(&id, refusal.code, refusal.message, refusal.data.as_ref());
                }
            },
            // The host gave a call up: it is not answered when it ends.
            Asked::Note { method, params } if method == "notifications/cancelled" => {
                self.running.cancel(&params["requestId"]);
            }
            Asked::Note { .. } | Asked::Nothing => {}
            Asked::Refused(code, why) => self.out.error(&Value::Null, code, why),
        }
    }

    /// A request of `era` answered: each era has its own way to open, and the tool is the same
    /// in both.
    fn answer(&self, era: Era, id: &Value, method: &str, params: &Value) {
        match (era, method) {
            (Era::Legacy, "initialize") => self.out.done(id, &era::initialized(params)),
            (Era::Legacy, "ping") => self.out.done(id, &json!({})),
            (Era::Modern, "server/discover") => self.out.done(id, &era::discovered()),
            // The one tool is the whole list: there is no page for a cursor to name.
            (_, "tools/list") if !params["cursor"].is_null() => {
                self.out.error(id, -32602, "no page is at that `cursor`");
            }
            (_, "tools/list") => (self.out).done(id, &era.listed(json!({"tools": [tool()]}))),
            (_, "tools/call") => self.call(era, id, params),
            (_, other) => (self.out).error(id, -32601, &format!("no method `{other}`")),
        }
    }

    /// A tool call, on a thread of its own, within the share of them there may be.
    fn call(&self, era: Era, id: &Value, params: &Value) {
        if params["name"].as_str() != Some(TOOL) {
            self.out
                .error(id, -32602, &format!("the one tool is `{TOOL}`"));
            return;
        }
        let args = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(args) if args.is_object() => args.clone(),
            Some(_) => {
                (self.out).error(id, -32602, "`arguments` is a JSON object");
                return;
            }
        };
        if !self.running.start(id) {
            let busy = failed("too many calls are running at once: wait for one");
            self.out.done(id, &era.complete(busy));
            return;
        }
        let request = window_params(&self.caller, &self.cwd, &args);
        let followed = running::followed(&self.out, &self.running, id, &params["_meta"]);
        let spawned = std::thread::Builder::new().name("mcp-call".into()).spawn({
            let (out, running, id) = (self.out.clone(), Arc::clone(&self.running), id.clone());
            move || {
                let answer = result(ask(Start::Spawn, "window/call", &request));
                drop(followed);
                if running.finish(&id) {
                    out.done(&id, &era.complete(answer));
                }
            }
        });
        if let Err(why) = spawned {
            self.running.finish(id);
            let said = failed(&format!("no thread for the call ({why})"));
            self.out.done(id, &era.complete(said));
        }
    }
}

/// The window tool as the running app describes it, else as this build does.
fn tool() -> Value {
    let described = ask(Start::Never, "api/describe", &json!({}))
        .ok()
        .map(|answer| answer.value["window"].clone())
        .filter(|window| window["schema"].is_object());
    let window = described.unwrap_or_else(
        || json!({"description": crate::desk::description(), "schema": crate::desk::schema()}),
    );
    named(&window)
}

/// The tool as a host lists it, from what the app says of it (`description`, `schema`). One
/// tool opens the web, runs programs and removes cards, so its hints say the most any call of
/// it may do: a host never runs two at once nor repeats one by itself on their word.
fn named(window: &Value) -> Value {
    json!({
        "name": TOOL,
        "title": "LLX Desk Window",
        "description": window["description"],
        "inputSchema": window["schema"],
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": true,
            "idempotentHint": false,
            "openWorldHint": true,
        },
    })
}

/// The server's answer as a tool result: its text, a picture as image content, a failure as a
/// result the model sees (`isError`), never a protocol error.
fn result(answer: Result<Answer, Trouble>) -> Value {
    match answer {
        Ok(answer) => {
            let text = answer.value["text"].as_str().unwrap_or_default();
            let mut content = vec![];
            if !text.is_empty() {
                content.push(json!({"type": "text", "text": text}));
            }
            if let Some(blob) = &answer.blob {
                let data = base64::engine::general_purpose::STANDARD.encode(&blob.bytes);
                content.push(json!({"type": "image", "data": data, "mimeType": blob.mime}));
            }
            if content.is_empty() {
                content.push(json!({"type": "text", "text": "Done."}));
            }
            json!({"content": content})
        }
        Err(Trouble::None) => failed("LLX Desk is not running"),
        Err(Trouble::Refused(why) | Trouble::Lost(why)) => failed(&why),
    }
}

fn failed(message: &str) -> Value {
    json!({"content": [{"type": "text", "text": message}], "isError": true})
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
