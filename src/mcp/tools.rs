//! A tool call as the daemon takes it, and the daemon's answer as a tool result: the arguments
//! become a `window` call, a picture becomes MCP image content, and a failure is a result with
//! `isError` (so the model sees it and can correct itself), not a protocol error.

use base64::Engine as _;
use serde_json::{Map, Value, json};

use super::relay::Failure;
use crate::client::Answer;

/// Which request of the daemon a tool makes.
pub(super) enum Method {
    /// A call of the `window` tool, with these arguments.
    Window(Value),
    /// What the person sent that was not read.
    Said,
}

/// The tool's arguments as the daemon's request; none for a name that is no tool.
pub(super) fn request(name: &str, arguments: &Value) -> Option<Method> {
    let given = arguments.as_object().cloned().unwrap_or_default();
    let with = |action: &str| {
        let mut args = given.clone();
        args.insert("action".into(), json!(action));
        Method::Window(Value::Object(args))
    };
    match name {
        "open" => Some(with("open")),
        "act" => Some(with("interact")),
        "look" => {
            let mut args = Map::new();
            args.insert("action".into(), json!("interact"));
            for (from, to) in [("tab", "tab"), ("text", "read")] {
                if let Some(value) = given.get(from) {
                    args.insert(to.into(), value.clone());
                }
            }
            Some(Method::Window(Value::Object(args)))
        }
        "screenshot" => Some(with("screenshot")),
        "tabs" => {
            let action = given
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("list")
                .to_string();
            let mut args = given;
            args.remove("action");
            args.insert("action".into(), json!(action));
            Some(Method::Window(Value::Object(args)))
        }
        "comments" => Some(Method::Said),
        _ => None,
    }
}

/// The daemon's answer as a tool result.
pub(super) fn result(answer: Result<Answer, Failure>, empty: &str) -> Value {
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
                content.push(json!({"type": "text", "text": empty}));
            }
            json!({"content": content})
        }
        Err(Failure::Absent) => json!({"content": [{"type": "text", "text": empty}]}),
        Err(Failure::Refused(error)) => failed(&error.message),
        Err(Failure::Unavailable(why)) => failed(&format!(
            "llx-desk: {why}. The CLI is the fallback: `llx-desk window '<json>'` in a shell."
        )),
    }
}

/// A tool result that says the call failed.
pub(super) fn failed(message: &str) -> Value {
    json!({"content": [{"type": "text", "text": message}], "isError": true})
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
