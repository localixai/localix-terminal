//! The two eras of the protocol an MCP server speaks at once. A legacy host opens with
//! `initialize` and is answered in the revision it asks for; a modern one (2026-07-28) opens
//! with nothing: each request names its revision and the host's capabilities in `_meta`, is
//! served on its own, and its result says its type. Which one a request is of is read from the
//! request alone, so both are served on the one process.

use serde_json::{Value, json};

/// The revision served request by request.
pub(super) const MODERN: &str = "2026-07-28";
/// The revisions that open with `initialize`, newest first.
pub(super) const LEGACY: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Where a modern request names its revision, and the capabilities of the host that sent it.
const REVISION: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

/// How long a host may keep what the server says of itself, and its tool list (the app's
/// description of the tool changes only with the app), in milliseconds.
const DISCOVERED_FOR: u64 = 3_600_000;
const LISTED_FOR: u64 = 300_000;

/// What the server is for, which a host may keep in view while the tool itself waits to be
/// looked up: every kind of tab and what one would ask of it, and nothing the tool's own
/// description says.
const INSTRUCTIONS: &str = "A window the person watches, holding this project's tabs: web pages \
    (check a page you build, play a UI flow, size it as a phone, take a screenshot), real \
    terminals (a REPL, an installer's questions, a login, another agent's interface) and the \
    project's kanban board of cards (plan work, take a card, hand it in for the person's review). \
    The one tool, `window`, opens, acts in, pictures and closes them.";

/// The era a request is served in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Era {
    Legacy,
    Modern,
}

/// A request refused before it is served: the error's code, its words and what it carries.
#[derive(Debug, PartialEq)]
pub(super) struct Refusal {
    pub(super) code: i64,
    pub(super) message: &'static str,
    pub(super) data: Option<Value>,
}

/// The era of the request `method` with `params`: modern when its `_meta` names a revision,
/// which must be the modern one and come with the host's capabilities; else legacy. Discovery
/// is modern only, so one that names no revision is malformed.
pub(super) fn of(method: &str, params: &Value) -> Result<Era, Refusal> {
    let meta = &params["_meta"];
    let refused = |code, message, data| {
        Err(Refusal {
            code,
            message,
            data,
        })
    };
    match meta.get(REVISION) {
        None if method == "server/discover" => {
            refused(-32602, "`_meta` names no protocol version", None)
        }
        None => Ok(Era::Legacy),
        Some(asked) if asked == MODERN => match meta[CAPABILITIES].is_object() {
            true => Ok(Era::Modern),
            false => refused(-32602, "`_meta` names no client capabilities", None),
        },
        Some(asked) => refused(
            -32022,
            "Unsupported protocol version",
            Some(json!({"supported": supported(), "requested": asked})),
        ),
    }
}

/// Every revision served, newest first.
fn supported() -> Vec<&'static str> {
    std::iter::once(MODERN).chain(LEGACY).collect()
}

fn server_info() -> Value {
    json!({"name": "llx-desk", "title": "LLX Desk", "version": env!("CARGO_PKG_VERSION")})
}

/// The `initialize` result: the legacy revision to speak (the one asked when it is known, else
/// the newest), and what the server offers.
pub(super) fn initialized(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or_default();
    let revision = (LEGACY.iter())
        .find(|known| **known == asked)
        .unwrap_or(&LEGACY[0]);
    json!({
        "protocolVersion": revision,
        "capabilities": {"tools": {}},
        "serverInfo": server_info(),
        "instructions": INSTRUCTIONS,
    })
}

/// The `server/discover` result: the revisions served, and what the server offers.
pub(super) fn discovered() -> Value {
    json!({
        "resultType": "complete",
        "supportedVersions": supported(),
        "capabilities": {"tools": {}},
        "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
        "instructions": INSTRUCTIONS,
        "ttlMs": DISCOVERED_FOR,
        "cacheScope": "public",
    })
}

impl Era {
    /// `result` as this era has a finished one: a modern result says that it is complete.
    pub(super) fn complete(self, mut result: Value) -> Value {
        if self == Self::Modern {
            result["resultType"] = "complete".into();
        }
        result
    }

    /// `result`, a list, as this era has one: a modern list says how long it may be kept, and
    /// that it is the same for every caller.
    pub(super) fn listed(self, result: Value) -> Value {
        let mut result = self.complete(result);
        if self == Self::Modern {
            result["ttlMs"] = LISTED_FOR.into();
            result["cacheScope"] = "public".into();
        }
        result
    }
}

#[cfg(test)]
#[path = "era_tests.rs"]
mod tests;
