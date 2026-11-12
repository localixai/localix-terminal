//! OpenCode: skills in `<its config>/skills` (`XDG_CONFIG_HOME`, else `~/.config/opencode`);
//! MCP servers in its `opencode.json`, which it has no command to write: the entry is written
//! there by setup's one writer of that file (the app's own entry's), in the shape of the
//! OpenCode installed (its first configuration: `mcp.<key>`, with `enabled`). It takes no
//! plugins of a marketplace.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::server::{Seen, Servers, Wanted};
use super::{Agent, var};
use crate::catalog::{Env, Runs};
use crate::setup::reach;

pub(super) static OPENCODE: Agent = Agent {
    id: "opencode",
    name: "OpenCode",
    program: "opencode",
    skills,
    shared: "",
    after: "start OpenCode again to use it",
    servers: &Mcp,
    plugins: None,
};

fn config(env: &Env) -> PathBuf {
    reach::config_file("opencode", &env.home, var(env))
        .unwrap_or_else(|| env.home.join(".config/opencode/opencode.json"))
}

fn skills(env: &Env) -> PathBuf {
    let config = config(env);
    config
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("skills")
}

/// The entry OpenCode keeps for `wanted`: a local one's command line is one list.
pub(super) fn entry(wanted: &Wanted<'_>) -> Value {
    let pairs = |pairs: &[(String, String)]| -> serde_json::Map<String, Value> {
        (pairs.iter())
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect()
    };
    let (mut entry, key, given) = match wanted.runs {
        Runs::Command { command, args } => {
            let line: Vec<&String> = std::iter::once(command).chain(args).collect();
            let entry = json!({"type": "local", "command": line, "enabled": true});
            (entry, "environment", pairs(&wanted.env))
        }
        Runs::Remote { url, .. } => {
            let entry = json!({"type": "remote", "url": url, "enabled": true});
            (entry, "headers", pairs(&wanted.headers))
        }
    };
    if !given.is_empty() {
        entry[key] = Value::Object(given);
    }
    entry
}

/// OpenCode's file with entry `key` set to `entry`, or taken out; written only when it changed.
fn write(env: &Env, key: &str, entry: Option<Value>) -> Result<(), String> {
    let file = config(env);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let written = reach::opencode_server(&text, key, entry)?;
    match written == text {
        true => Ok(()),
        false => reach::write_over(&file, &written),
    }
}

struct Mcp;

impl Servers for Mcp {
    fn takes(&self, _: &Runs) -> bool {
        true
    }

    fn file(&self, env: &Env) -> PathBuf {
        config(env)
    }

    fn seen(&self, text: &str, key: &str) -> Option<Seen> {
        Seen::of(reach::json(text)?.get("mcp")?.get(key)?)
    }

    fn add(&self, _: &Agent, env: &Env, wanted: &Wanted<'_>) -> Result<(), String> {
        write(env, &wanted.key, Some(entry(wanted)))
    }

    fn take_out(&self, _: &Agent, env: &Env, key: &str) -> Result<(), String> {
        write(env, key, None)
    }
}

#[cfg(test)]
#[path = "opencode_tests.rs"]
mod tests;
