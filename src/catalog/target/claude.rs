//! Claude Code: skills in `<its folder>/skills` (`CLAUDE_CONFIG_DIR`, else `~/.claude`); MCP
//! servers at user scope, in its `.claude.json`, put in and taken out by `claude mcp add-json`
//! and `claude mcp remove`; plugins of Claude marketplaces by `claude plugin … --json`, whose
//! records (`plugins/installed_plugins.json`, `plugins/known_marketplaces.json`, shapes Claude
//! Code does not document) are read leniently: anything else reads as not installed.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::plugin::{Plugin, Plugins, by_status, claude_outcome, words};
use super::server::{Seen, Servers, Wanted};
use super::{Agent, var};
use crate::catalog::{Env, PluginFormat, Runs};
use crate::setup::reach::{self, Ran};

/// How long an `mcp` command may take.
const MOST: Duration = Duration::from_secs(60);

pub(super) static CLAUDE: Agent = Agent {
    id: "claude",
    name: "Claude Code",
    program: "claude",
    skills,
    shared: "",
    after: "start a new Claude Code session to use it",
    servers: &Mcp,
    plugins: Some(&Market),
};

fn skills(env: &Env) -> PathBuf {
    reach::claude_dir(&env.home, var(env)).join("skills")
}

/// A record of Claude Code's under its plugins folder, read as JSON.
fn record(env: &Env, name: &str) -> Option<Value> {
    let file = reach::claude_dir(&env.home, var(env))
        .join("plugins")
        .join(name);
    reach::json(&std::fs::read_to_string(file).ok()?)
}

/// The entry Claude Code keeps for `wanted`.
pub(super) fn entry(wanted: &Wanted<'_>) -> Value {
    let pairs = |pairs: &[(String, String)]| -> serde_json::Map<String, Value> {
        (pairs.iter())
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect()
    };
    let mut entry = match wanted.runs {
        Runs::Command { command, args } => {
            json!({"type": "stdio", "command": command, "args": args})
        }
        Runs::Remote { url, sse } => json!({"type": if *sse { "sse" } else { "http" }, "url": url}),
    };
    let (key, given) = match wanted.runs {
        Runs::Command { .. } => ("env", pairs(&wanted.env)),
        Runs::Remote { .. } => ("headers", pairs(&wanted.headers)),
    };
    if !given.is_empty() {
        entry[key] = Value::Object(given);
    }
    entry
}

struct Mcp;

impl Servers for Mcp {
    fn takes(&self, _: &Runs) -> bool {
        true
    }

    fn file(&self, env: &Env) -> PathBuf {
        reach::claude_json(&env.home, var(env))
    }

    fn seen(&self, text: &str, key: &str) -> Option<Seen> {
        Seen::of(reach::json(text)?.get("mcpServers")?.get(key)?)
    }

    fn add(&self, agent: &Agent, env: &Env, wanted: &Wanted<'_>) -> Result<(), String> {
        let entry = entry(wanted).to_string();
        let words = ["mcp", "add-json", "--scope", "user", &wanted.key, &entry];
        by_status(&agent.run(env, &words, MOST)?)
    }

    fn take_out(&self, agent: &Agent, env: &Env, key: &str) -> Result<(), String> {
        by_status(&agent.run(env, &["mcp", "remove", "--scope", "user", key], MOST)?)
    }
}

struct Market;

impl Plugins for Market {
    fn takes(&self, format: PluginFormat) -> bool {
        match format {
            PluginFormat::Claude => true,
            PluginFormat::Codex => false,
        }
    }

    fn known(&self, env: &Env, name: &str) -> bool {
        record(env, "known_marketplaces.json").is_some_and(|known| known.get(name).is_some())
    }

    fn add_market(&self, from: &str) -> Vec<String> {
        words(&["plugin", "marketplace", "add", from, "--json"])
    }

    fn install(&self, id: &str) -> Vec<String> {
        words(&["plugin", "install", id, "--scope", "user", "--json"])
    }

    fn uninstall(&self, id: &str) -> Vec<String> {
        words(&["plugin", "uninstall", id, "--scope", "user", "--json"])
    }

    fn outcome(&self, ran: &Ran) -> Result<(), String> {
        claude_outcome(ran)
    }

    fn has(&self, env: &Env, plugin: &Plugin<'_>) -> Option<String> {
        installed(&record(env, "installed_plugins.json")?, &plugin.id())
    }

    fn after(&self) -> &'static str {
        "run /reload-plugins in Claude Code, or start a new session"
    }
}

/// The version of plugin `id` at user scope in Claude Code's `installed_plugins.json`.
pub(super) fn installed(records: &Value, id: &str) -> Option<String> {
    let copies = records.get("plugins")?.get(id)?.as_array()?;
    let mine = (copies.iter()).find(|copy| copy["scope"] == "user")?;
    Some(mine["version"].as_str().unwrap_or_default().to_string())
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
