//! The messages of an MCP server on stdio: a line of the host's read as the protocol says (a
//! request, a notification, or no message at all), and the one writer of stdout, which writes
//! whole messages in order.

use std::io::Write;
use std::sync::mpsc::{SyncSender, sync_channel};

use serde_json::{Value, json};

/// The one writer of stdout: whole messages, in order.
#[derive(Clone)]
pub(super) struct Out(SyncSender<String>);

impl Out {
    pub(super) fn spawn(
        mut stream: impl Write + Send + 'static,
    ) -> std::io::Result<(Self, std::thread::JoinHandle<()>)> {
        let (sender, queue) = sync_channel::<String>(256);
        let writer = std::thread::Builder::new()
            .name("mcp-write".into())
            .spawn(move || {
                for line in queue {
                    if writeln!(stream, "{line}")
                        .and_then(|()| stream.flush())
                        .is_err()
                    {
                        return;
                    }
                }
            })?;
        Ok((Self(sender), writer))
    }

    pub(super) fn send(&self, message: &Value) {
        let _ = self.0.send(message.to_string());
    }

    pub(super) fn done(&self, id: &Value, result: &Value) {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    pub(super) fn error(&self, id: &Value, code: i64, message: &str) {
        self.refuse(id, code, message, None);
    }

    /// An error with what the protocol has it carry (`data`), when it has.
    pub(super) fn refuse(&self, id: &Value, code: i64, message: &str, data: Option<&Value>) {
        let mut error = json!({"code": code, "message": message});
        if let Some(data) = data {
            error["data"] = data.clone();
        }
        self.send(&json!({"jsonrpc": "2.0", "id": id, "error": error}));
    }
}

/// What a line of the host's asks.
#[derive(Debug)]
pub(super) enum Asked {
    /// A request: answered under its id.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A notification: never answered.
    Note { method: String, params: Value },
    /// An answer to nothing this server asked: ignored.
    Nothing,
    /// No message of the protocol: the error's code and words.
    Refused(i64, &'static str),
}

/// A line read as the protocol says: JSON that does not parse is a parse error, and JSON that
/// is no request (a batch, which the revisions since 2025-06-18 took out; an id that is neither
/// a string nor a number) an invalid one.
pub(super) fn asked(line: &str) -> Asked {
    let message = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(message)) => message,
        Ok(Value::Array(_)) => return Asked::Refused(-32600, "one message per line, not a batch"),
        Ok(_) => return Asked::Refused(-32600, "a message is a JSON object"),
        Err(_) => return Asked::Refused(-32700, "a message is one JSON object per line"),
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Asked::Nothing;
    };
    let method = method.to_string();
    match message.get("id") {
        None => Asked::Note { method, params },
        Some(id) if id.is_string() || id.is_number() => Asked::Request {
            id: id.clone(),
            method,
            params,
        },
        Some(_) => Asked::Refused(-32600, "a request's id is a string or a number"),
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
