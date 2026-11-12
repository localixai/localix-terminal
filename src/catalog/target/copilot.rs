//! GitHub Copilot CLI: skills in `<its home>/skills` (`COPILOT_HOME`, else `~/.copilot`); MCP
//! servers in its `mcp-config.json`, put in by `copilot mcp add` (a program after `--`, an
//! address with `--transport`) and taken out by `copilot mcp remove`; plugins of Claude
//! marketplaces, which it reads, by `copilot plugin`. It records the marketplaces it was told of
//! and the plugins it has on in its `settings.json` (`extraKnownMarketplaces`,
//! `enabledPlugins`), and names no version there.

use std::path::PathBuf;
use std::time::Duration;

use super::plugin::{Plugin, Plugins, by_status, words};
use super::server::{Seen, Servers, Wanted};
use super::{Agent, var};
use crate::catalog::{Env, PluginFormat, Runs};
use crate::setup::reach::{self, Ran};

const MOST: Duration = Duration::from_secs(60);

/// The marketplace it has from the start, which no settings list.
const BUILT_IN: &str = "awesome-copilot";

pub(super) static COPILOT: Agent = Agent {
    id: "copilot",
    name: "GitHub Copilot",
    program: "copilot",
    skills,
    shared: "",
    after: "start Copilot again to use it",
    servers: &Mcp,
    plugins: Some(&Market),
};

/// Its MCP servers' file, in its home.
fn config(env: &Env) -> PathBuf {
    reach::config_file("copilot", &env.home, var(env))
        .unwrap_or_else(|| env.home.join(".copilot/mcp-config.json"))
}

/// Its home: the folder of that file.
fn home(env: &Env) -> PathBuf {
    let config = config(env);
    config.parent().map(PathBuf::from).unwrap_or_default()
}

fn skills(env: &Env) -> PathBuf {
    home(env).join("skills")
}

/// The words `copilot` is run with to add `wanted`.
pub(super) fn add_words(wanted: &Wanted<'_>) -> Vec<String> {
    let mut words = vec!["mcp".to_string(), "add".into()];
    match wanted.runs {
        Runs::Command { command, args } => {
            for (name, value) in &wanted.env {
                words.extend(["--env".to_string(), format!("{name}={value}")]);
            }
            words.extend([wanted.key.clone(), "--".into(), command.clone()]);
            words.extend(args.iter().cloned());
        }
        Runs::Remote { url, sse } => {
            words.extend([
                "--transport".to_string(),
                if *sse { "sse" } else { "http" }.into(),
            ]);
            for (name, value) in &wanted.headers {
                words.extend(["--header".to_string(), format!("{name}: {value}")]);
            }
            words.extend([wanted.key.clone(), url.clone()]);
        }
    }
    words
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
        Seen::of(reach::json(text)?.get("mcpServers")?.get(key)?)
    }

    fn add(&self, agent: &Agent, env: &Env, wanted: &Wanted<'_>) -> Result<(), String> {
        let words = add_words(wanted);
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        by_status(&agent.run(env, &words, MOST)?)
    }

    fn take_out(&self, agent: &Agent, env: &Env, key: &str) -> Result<(), String> {
        by_status(&agent.run(env, &["mcp", "remove", key], MOST)?)
    }
}

struct Market;

/// Copilot's `settings.json`, read.
fn settings(env: &Env) -> Option<serde_json::Value> {
    reach::json(&std::fs::read_to_string(home(env).join("settings.json")).ok()?)
}

impl Plugins for Market {
    fn takes(&self, format: PluginFormat) -> bool {
        match format {
            PluginFormat::Claude => true,
            PluginFormat::Codex => false,
        }
    }

    fn known(&self, env: &Env, name: &str) -> bool {
        name == BUILT_IN
            || settings(env).is_some_and(|it| it["extraKnownMarketplaces"].get(name).is_some())
    }

    fn add_market(&self, from: &str) -> Vec<String> {
        words(&["plugin", "marketplace", "add", from])
    }

    fn install(&self, id: &str) -> Vec<String> {
        words(&["plugin", "install", id])
    }

    fn uninstall(&self, id: &str) -> Vec<String> {
        words(&["plugin", "uninstall", id])
    }

    /// It may end well and still say it failed.
    fn outcome(&self, ran: &Ran) -> Result<(), String> {
        by_status(ran)?;
        match ran.out.trim_start().starts_with("Failed") {
            true => Err(ran.out.trim().to_string()),
            false => Ok(()),
        }
    }

    fn has(&self, env: &Env, plugin: &Plugin<'_>) -> Option<String> {
        let on = settings(env)?["enabledPlugins"][plugin.id()] == true;
        on.then(String::new)
    }

    fn after(&self) -> &'static str {
        "start Copilot again to use it"
    }
}

#[cfg(test)]
#[path = "copilot_tests.rs"]
mod tests;
