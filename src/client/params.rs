//! The params of a `window/call`, as every client builds them.

use std::path::PathBuf;

use serde_json::{Value, json};

/// The project the agent's host says it works in (`CLAUDE_PROJECT_DIR`): the owner of a call is
/// the project around it, so the command line, the hooks and the MCP server agree on it.
pub fn host_project() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_PROJECT_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// A `window/call` request's params. The environment goes with a call that may start a program;
/// the OpenRouter key (`key::find`) with a call that has a goal.
pub fn window_params(owner: &str, cwd: &str, args: &Value) -> Value {
    let flat = args.to_string();
    let starts = flat.contains("\"open\"") || flat.contains("\"command\"");
    let env: Option<serde_json::Map<String, Value>> = starts.then(|| {
        std::env::vars()
            .map(|(name, value)| (name, json!(value)))
            .collect()
    });
    let key = flat.contains("\"goal\"").then(crate::key::find).flatten();
    json!({
        "owner": owner,
        "cwd": cwd,
        "env": env,
        "key": key,
        "args": args,
        "idempotencyKey": crate::token(),
    })
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
