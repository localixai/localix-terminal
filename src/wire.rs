//! The protocol between a client and the app: JSON-RPC 2.0, one message per line. A response
//! may carry bytes (a screenshot): its result announces them as `binary: {length, mime}`, and
//! exactly that many raw bytes follow its line, on the same connection, as part of the same
//! write. The protocol grows by optional fields and new methods; both sides ignore what they do
//! not know.

mod line;
pub(crate) mod peer;
pub mod pipe;

use std::io::{self, BufRead, Read};

use serde_json::{Value, json};

pub use line::respond;

/// The protocol version this build speaks. A breaking change is a new number.
pub const PROTOCOL: u32 = 3;

/// The longest line read: a request or a response without its bytes.
const MAX_LINE: u64 = 8 << 20;
/// The most bytes a response may announce: a screenshot, never a video.
const MAX_BINARY: u64 = 64 << 20;

/// JSON-RPC's own error codes, and ours.
pub mod code {
    pub const PARSE: i64 = -32700;
    pub const INVALID: i64 = -32600;
    pub const NO_METHOD: i64 = -32601;
    pub const BAD_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    /// The call failed; the next may not.
    pub const FAILED: i64 = -32000;
    /// `initialize` has not come first.
    pub const NOT_INITIALIZED: i64 = -32002;
    /// The app is stopping, or too busy to take the call.
    pub const UNAVAILABLE: i64 = -32003;
}

/// An error as a response carries it.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Bytes that travel after a response: what they are, and the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// One message, as JSON-RPC names the three kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Response {
        id: Value,
        outcome: Result<Value, RpcError>,
    },
}

/// A message with the bytes that came after it, if it announced any.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub message: Message,
    pub blob: Option<Blob>,
}

/// What the next read found.
#[derive(Debug)]
pub enum Next {
    /// The stream ended.
    End,
    /// A line that is no message: answer it with this error and read on (the lines resync).
    Bad(RpcError),
    Got(Frame),
}

impl Message {
    pub fn request(id: u64, method: &str, params: Value) -> Self {
        Self::Request {
            id: json!(id),
            method: method.to_string(),
            params,
        }
    }

    pub fn notification(method: &str, params: Value) -> Self {
        Self::Notification {
            method: method.to_string(),
            params,
        }
    }

    pub fn done(id: Value, result: Value) -> Self {
        Self::Response {
            id,
            outcome: Ok(result),
        }
    }

    pub fn failed(id: Value, error: RpcError) -> Self {
        Self::Response {
            id,
            outcome: Err(error),
        }
    }

    /// The message as bytes to write in one piece: its line, then `blob`'s bytes when the message
    /// is a response with a result to announce them in.
    pub fn encode(&self, blob: Option<&Blob>) -> Vec<u8> {
        match self {
            Self::Request { id, method, params } => line::request(id, method, params),
            Self::Notification { method, params } => line::notification(method, params),
            Self::Response { id, outcome } => respond(id, outcome, blob),
        }
    }
}

/// The next message and its bytes. An I/O error ends the connection; a line that is no message
/// does not.
pub fn read_frame<R: BufRead>(input: &mut R) -> io::Result<Next> {
    let mut line = Vec::new();
    if Read::take(&mut *input, MAX_LINE).read_until(b'\n', &mut line)? == 0 {
        return Ok(Next::End);
    }
    if line.last() != Some(&b'\n') && line.len() as u64 >= MAX_LINE {
        // Read past the rest of it, keeping none: the connection stays usable, and the sender
        // is told why instead of finding the app gone.
        skip_line(input)?;
        return Ok(Next::Bad(RpcError::new(
            code::INVALID,
            "a message is at most 8 MiB",
        )));
    }
    let value: Value = match serde_json::from_slice(&line) {
        Ok(value) => value,
        Err(error) => return Ok(Next::Bad(RpcError::new(code::PARSE, error.to_string()))),
    };
    let message = match parse(value) {
        Ok(message) => message,
        Err(error) => return Ok(Next::Bad(error)),
    };
    let blob = match &message {
        Message::Response {
            outcome: Ok(result),
            ..
        } => announced(input, result)?,
        Message::Request { .. } | Message::Notification { .. } | Message::Response { .. } => None,
    };
    Ok(Next::Got(Frame { message, blob }))
}

/// Read up to the end of the current line, keeping nothing.
fn skip_line<R: BufRead>(input: &mut R) -> io::Result<()> {
    loop {
        let seen = input.fill_buf()?;
        if seen.is_empty() {
            return Ok(());
        }
        match seen.iter().position(|byte| *byte == b'\n') {
            Some(at) => {
                input.consume(at + 1);
                return Ok(());
            }
            None => {
                let all = seen.len();
                input.consume(all);
            }
        }
    }
}

/// The bytes a result announced, read.
fn announced<R: BufRead>(input: &mut R, result: &Value) -> io::Result<Option<Blob>> {
    let Some(info) = result.get("binary") else {
        return Ok(None);
    };
    let length = info.get("length").and_then(Value::as_u64).unwrap_or(0);
    if length > MAX_BINARY {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bytes too long"));
    }
    let mut bytes = Vec::new();
    Read::take(&mut *input, length).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let mime = info
        .get("mime")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    Ok(Some(Blob {
        mime: mime.to_string(),
        bytes,
    }))
}

fn parse(value: Value) -> Result<Message, RpcError> {
    let invalid = |why: &str| RpcError::new(code::INVALID, why);
    let Value::Object(mut fields) = value else {
        return Err(invalid("a message is a JSON object"));
    };
    if fields.get("jsonrpc") != Some(&json!("2.0")) {
        return Err(invalid(
            "not JSON-RPC 2.0: an older app speaks another protocol",
        ));
    }
    let sent = fields.remove("id");
    if sent
        .as_ref()
        .is_some_and(|id| !id.is_null() && !id.is_string() && !id.is_number())
    {
        return Err(invalid("a message id is a string or a number"));
    }
    // A null id is JSON-RPC's own for an error that could not name its request (a line that did
    // not parse, a connection refused): kept for an error response, nothing else.
    let null_error = if sent == Some(Value::Null) {
        fields.remove("error").map(|error| error_of(&error))
    } else {
        None
    };
    let id = sent.filter(|id| id.is_string() || id.is_number());
    let params = fields.remove("params").unwrap_or(Value::Null);
    match (fields.remove("method"), id) {
        (Some(Value::String(method)), Some(id)) => Ok(Message::Request { id, method, params }),
        (Some(Value::String(method)), None) => Ok(Message::Notification { method, params }),
        (Some(_), _) => Err(invalid("`method` is a string")),
        (None, Some(id)) => match (fields.remove("result"), fields.remove("error")) {
            (_, Some(error)) => Ok(Message::failed(id, error_of(&error))),
            (Some(result), None) => Ok(Message::done(id, result)),
            (None, None) => Err(invalid("a response has a `result` or an `error`")),
        },
        (None, None) => match null_error {
            Some(error) => Ok(Message::failed(Value::Null, error)),
            None => Err(invalid("a message has a `method` or an `id`")),
        },
    }
}

fn error_of(error: &Value) -> RpcError {
    RpcError::new(
        error
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or(code::INTERNAL),
        error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("an error without a message"),
    )
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
