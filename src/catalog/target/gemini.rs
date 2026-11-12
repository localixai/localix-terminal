//! Gemini CLI: skills in `~/.agents/skills`, the folder Codex reads too; MCP servers in the
//! `mcpServers` of its user settings (`GEMINI_CLI_HOME`), put in by `gemini mcp add --scope
//! user` (a program's arguments after `--`, so none is taken for one of its own options; an
//! address with `--transport http` or `sse`) and taken out by `gemini mcp remove --scope
//! user`. Its extensions are no plugins of a marketplace: it takes none.

use std::path::PathBuf;
use std::time::Duration;

use super::plugin::by_status;
use super::server::{Seen, Servers, Wanted};
use super::{Agent, codex, var};
use crate::catalog::{Env, Runs};
use crate::setup::reach;

const MOST: Duration = Duration::from_secs(60);

pub(super) static GEMINI: Agent = Agent {
    id: "gemini",
    name: "Gemini CLI",
    program: "gemini",
    skills: codex::shared_skills,
    shared: "Codex and Gemini CLI",
    after: "start Gemini CLI again to use it",
    servers: &Mcp,
    plugins: None,
};

/// The words `gemini` is run with to add `wanted`.
pub(super) fn add_words(wanted: &Wanted<'_>) -> Vec<String> {
    let mut words: Vec<String> = ["mcp", "add", "--scope", "user"].map(String::from).into();
    match wanted.runs {
        Runs::Command { command, args } => {
            for (name, value) in &wanted.env {
                words.extend(["--env".to_string(), format!("{name}={value}")]);
            }
            words.extend([wanted.key.clone(), command.clone()]);
            if !args.is_empty() {
                words.push("--".into());
                words.extend(args.iter().cloned());
            }
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
        reach::config_file("gemini", &env.home, var(env))
            .unwrap_or_else(|| env.home.join(".gemini/settings.json"))
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
        by_status(&agent.run(env, &["mcp", "remove", "--scope", "user", key], MOST)?)
    }
}

#[cfg(test)]
#[path = "gemini_tests.rs"]
mod tests;
