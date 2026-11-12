//! Codex: skills in `~/.agents/skills`, the folder Gemini CLI reads too; MCP servers as
//! `[mcp_servers.<key>]` in its `config.toml` (`CODEX_HOME`), put in by `codex mcp add` (a
//! program with its `--env`, or a streamable address with `--url`; it takes no server-sent
//! events) and taken out by `codex mcp remove`. Its command takes no headers: a remote server's
//! are written after it as the table `[mcp_servers.<key>.http_headers]`. Plugins of either
//! format of marketplace (it reads Claude's too) by `codex plugin … --json`; it records one as
//! `[plugins."name@marketplace"]` and keeps its files in `plugins/cache/<marketplace>/<name>/
//! <version>`. The trust of a plugin's hooks is the person's to give in Codex, never written.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use super::plugin::{Plugin, Plugins, by_status, words};
use super::server::{Seen, Servers, Wanted};
use super::{Agent, var};
use crate::catalog::{Env, PluginFormat, Runs};
use crate::setup::reach::{self, Ran};

const MOST: Duration = Duration::from_secs(60);

pub(super) static CODEX: Agent = Agent {
    id: "codex",
    name: "Codex",
    program: "codex",
    skills: shared_skills,
    shared: "Codex and Gemini CLI",
    after: "start Codex again to use it",
    servers: &Mcp,
    plugins: Some(&Market),
};

/// The skills folder Codex and Gemini CLI both read: `~/.agents/skills`.
pub(super) fn shared_skills(env: &Env) -> PathBuf {
    env.home.join(".agents/skills")
}

/// Codex's `config.toml`.
fn config(env: &Env) -> PathBuf {
    reach::config_file("codex", &env.home, var(env))
        .unwrap_or_else(|| env.home.join(".codex/config.toml"))
}

/// The words `codex` is run with to add `wanted`.
pub(super) fn add_words(wanted: &Wanted<'_>) -> Vec<String> {
    let mut words = vec!["mcp".to_string(), "add".into(), wanted.key.clone()];
    match wanted.runs {
        Runs::Command { command, args } => {
            for (name, value) in &wanted.env {
                words.extend(["--env".to_string(), format!("{name}={value}")]);
            }
            words.extend(["--".to_string(), command.clone()]);
            words.extend(args.iter().cloned());
        }
        Runs::Remote { url, .. } => words.extend(["--url".to_string(), url.clone()]),
    }
    words
}

/// The table of a remote server's headers, as it is put after its entry; none without any.
pub(super) fn headers_table(wanted: &Wanted<'_>) -> Option<String> {
    if wanted.headers.is_empty() {
        return None;
    }
    // A JSON string is a TOML basic string, a key quoted as one too.
    let quoted = |text: &str| Value::String(text.into()).to_string();
    let lines: Vec<String> = (wanted.headers.iter())
        .map(|(name, value)| format!("{} = {}\n", quoted(name), quoted(value)))
        .collect();
    Some(format!(
        "\n[mcp_servers.{}.http_headers]\n{}",
        wanted.key,
        lines.concat()
    ))
}

/// The value `key` of table `table` in Codex's TOML, with the table's name quoted or not.
fn value(text: &str, table: &str, name: &str, key: &str) -> Option<Value> {
    reach::toml_value(text, &format!("{table}.{name}"), key)
        .or_else(|| reach::toml_value(text, &format!("{table}.\"{name}\""), key))
}

/// Server `key` in Codex's TOML.
pub(super) fn seen(text: &str, key: &str) -> Option<Seen> {
    let get = |field| value(text, "mcp_servers", key, field);
    let command = get("command").and_then(|it| it.as_str().map(String::from));
    let url = get("url").and_then(|it| it.as_str().map(String::from));
    let args = (get("args")
        .and_then(|it| it.as_array().cloned())
        .into_iter()
        .flatten())
    .filter_map(|word| word.as_str().map(String::from))
    .collect();
    (command.is_some() || url.is_some()).then_some(Seen { command, args, url })
}

struct Mcp;

impl Servers for Mcp {
    fn takes(&self, runs: &Runs) -> bool {
        !matches!(runs, Runs::Remote { sse: true, .. })
    }

    fn file(&self, env: &Env) -> PathBuf {
        config(env)
    }

    fn seen(&self, text: &str, key: &str) -> Option<Seen> {
        seen(text, key)
    }

    fn add(&self, agent: &Agent, env: &Env, wanted: &Wanted<'_>) -> Result<(), String> {
        let words = add_words(wanted);
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        by_status(&agent.run(env, &words, MOST)?)?;
        let Some(table) = headers_table(wanted) else {
            return Ok(());
        };
        let file = config(env);
        let text =
            std::fs::read_to_string(&file).map_err(|why| format!("{}: {why}", file.display()))?;
        reach::write_over(&file, &(text + &table))
    }

    fn take_out(&self, agent: &Agent, env: &Env, key: &str) -> Result<(), String> {
        by_status(&agent.run(env, &["mcp", "remove", key], MOST)?)
    }
}

struct Market;

impl Plugins for Market {
    fn takes(&self, format: PluginFormat) -> bool {
        match format {
            PluginFormat::Claude | PluginFormat::Codex => true,
        }
    }

    fn known(&self, env: &Env, name: &str) -> bool {
        let text = std::fs::read_to_string(config(env)).unwrap_or_default();
        value(&text, "marketplaces", name, "source").is_some()
    }

    fn add_market(&self, from: &str) -> Vec<String> {
        words(&["plugin", "marketplace", "add", from, "--json"])
    }

    fn install(&self, id: &str) -> Vec<String> {
        words(&["plugin", "add", id, "--json"])
    }

    fn uninstall(&self, id: &str) -> Vec<String> {
        words(&["plugin", "remove", id, "--json"])
    }

    fn outcome(&self, ran: &Ran) -> Result<(), String> {
        by_status(ran)
    }

    fn has(&self, env: &Env, plugin: &Plugin<'_>) -> Option<String> {
        let file = config(env);
        let text = std::fs::read_to_string(&file).ok()?;
        let on = reach::toml_value(&text, &format!("plugins.\"{}\"", plugin.id()), "enabled");
        if on != Some(Value::Bool(true)) {
            return None;
        }
        let cache = (file.parent()?.join("plugins/cache"))
            .join(plugin.marketplace)
            .join(plugin.name);
        Some(newest(&cache).unwrap_or_default())
    }

    fn after(&self) -> &'static str {
        "start a new Codex session to use it; Codex asks you in /hooks to trust any hooks it has"
    }
}

/// The name of the newest folder in `dir`: the version of a plugin Codex keeps there.
fn newest(dir: &std::path::Path) -> Option<String> {
    (std::fs::read_dir(dir).ok()?.flatten())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
