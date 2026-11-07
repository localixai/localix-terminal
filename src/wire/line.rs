//! A message as the bytes of its line: built out of the message's own parts, so that nothing is
//! copied to be written (an answer kept for a retry, a screenshot's bytes).

use serde::Serialize;
use serde_json::{Value, json};

use super::{Blob, RpcError};

/// A request as bytes to write in one piece.
pub(super) fn request(id: &Value, method: &str, params: &Value) -> Vec<u8> {
    let sent = Line {
        id: Some(id),
        method: Some(method),
        params: Some(params),
        ..Line::new()
    };
    finish(&sent, None)
}

/// A notification as bytes to write in one piece.
pub(super) fn notification(method: &str, params: &Value) -> Vec<u8> {
    let sent = Line {
        method: Some(method),
        params: Some(params),
        ..Line::new()
    };
    finish(&sent, None)
}

/// A response as bytes to write in one piece (`Message::encode` of one), from its parts, so that
/// an answer kept for a retry is written without being copied first.
pub fn respond(id: &Value, outcome: &Result<Value, RpcError>, blob: Option<&Blob>) -> Vec<u8> {
    let sent = match outcome {
        Ok(result) => Line {
            id: Some(id),
            result: Some(Announcing { result, blob }),
            ..Line::new()
        },
        Err(error) => Line {
            id: Some(id),
            error: Some(Failure {
                code: error.code,
                message: &error.message,
            }),
            ..Line::new()
        },
    };
    finish(&sent, blob.filter(|_| outcome.is_ok()))
}

/// `line` as JSON and a newline, then the bytes that follow it.
fn finish(line: &Line, bytes: Option<&Blob>) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.map_or(256, |blob| blob.bytes.len() + 256));
    // A line of this shape always serializes: it holds no map with a non-string key.
    let _ = serde_json::to_writer(&mut out, line);
    out.push(b'\n');
    if let Some(blob) = bytes {
        out.extend_from_slice(&blob.bytes);
    }
    out
}

/// A message's line, built out of the message's own parts: nothing is copied to be written.
#[derive(Serialize)]
struct Line<'a> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Announcing<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Failure<'a>>,
}

impl Line<'_> {
    const fn new() -> Self {
        Self {
            jsonrpc: "2.0",
            id: None,
            method: None,
            params: None,
            result: None,
            error: None,
        }
    }
}

#[derive(Serialize)]
struct Failure<'a> {
    code: i64,
    message: &'a str,
}

/// A result as it is written: when bytes follow it and it is an object, with `binary` added.
struct Announcing<'a> {
    result: &'a Value,
    blob: Option<&'a Blob>,
}

impl Serialize for Announcing<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let (Some(blob), Some(fields)) = (self.blob, self.result.as_object()) else {
            return self.result.serialize(serializer);
        };
        let mut map = serializer.serialize_map(None)?;
        for (name, value) in fields.iter().filter(|(name, _)| *name != "binary") {
            map.serialize_entry(name, value)?;
        }
        map.serialize_entry(
            "binary",
            &json!({"length": blob.bytes.len(), "mime": blob.mime}),
        )?;
        map.end()
    }
}
