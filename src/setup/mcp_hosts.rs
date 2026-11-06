//! The window tool in the coding agents that take MCP servers (Codex, Gemini CLI, GitHub Copilot,
//! OpenCode, Hermes): an `llx-desk` server in each one's own configuration, run as
//! `<this program> mcp`. Each is put in and taken out with the agent's own commands
//! (`<agent> mcp add` / `remove`), where it has them for a local server; OpenCode has none, and its
//! JSON is written. Whether it is there is read from the agent's configuration file, which is
//! quick and needs none of its programs to start. Any other host is given the entry to paste
//! (`snippet`).

use std::path::Path;

use serde_json::{Value, json};

use super::State;
use super::shell::{login_path, run_with, which};

/// The name of the server in an agent's configuration.
pub(super) const NAME: &str = "llx-desk";
/// How long an agent's `mcp` command may take (Hermes connects to the server to list its tools).
const MOST: std::time::Duration = std::time::Duration::from_secs(60);

/// How an agent's configuration names the server's command.
#[derive(Debug, Clone, Copy)]
enum Kept {
    /// JSON: the command (a string) at this path.
    Json(&'static [&'static str]),
    /// JSON: the command line (an array, the program first) at this path.
    JsonLine(&'static [&'static str]),
    /// TOML: `command` in the `[mcp_servers.llx-desk]` table.
    Toml,
    /// YAML: `command:` under `llx-desk:`.
    Yaml,
}

/// A coding agent that takes MCP servers.
#[derive(Debug, Clone, Copy)]
pub(super) struct Host {
    /// Its integration's id, its agent's (agents.rs).
    pub(super) id: &'static str,
    /// Its program, on the person's PATH.
    program: &'static str,
    /// Its configuration, under the home directory, and how it keeps the server.
    config: &'static str,
    kept: Kept,
    /// Its command that adds the server (`{exe}`: this program), none when it has none; what it
    /// is answered when it asks.
    add: Option<&'static [&'static str]>,
    answer: Option<&'static str>,
    /// Its command that takes the server out.
    remove: Option<&'static [&'static str]>,
    /// What to do after, in words.
    after: &'static str,
}

/// Every agent that takes MCP servers, as integrations.rs lists them.
pub(super) const HOSTS: [Host; 5] = [
    Host {
        id: "codex",
        program: "codex",
        config: ".codex/config.toml",
        kept: Kept::Toml,
        add: Some(&["mcp", "add", NAME, "--", "{exe}", "mcp"]),
        answer: None,
        remove: Some(&["mcp", "remove", NAME]),
        after: "start Codex again to use it",
    },
    Host {
        id: "gemini",
        program: "gemini",
        config: ".gemini/settings.json",
        kept: Kept::Json(&["mcpServers", NAME, "command"]),
        add: Some(&["mcp", "add", "--scope", "user", NAME, "{exe}", "mcp"]),
        answer: None,
        remove: Some(&["mcp", "remove", "--scope", "user", NAME]),
        after: "start Gemini CLI again to use it",
    },
    Host {
        id: "copilot",
        program: "copilot",
        config: ".copilot/mcp-config.json",
        kept: Kept::Json(&["mcpServers", NAME, "command"]),
        add: Some(&["mcp", "add", NAME, "--", "{exe}", "mcp"]),
        answer: None,
        remove: Some(&["mcp", "remove", NAME]),
        after: "start Copilot again to use it",
    },
    Host {
        id: "opencode",
        program: "opencode",
        config: ".config/opencode/opencode.json",
        kept: Kept::JsonLine(&["mcp", NAME, "command"]),
        add: None,
        answer: None,
        remove: None,
        after: "start OpenCode again to use it",
    },
    Host {
        id: "hermes",
        program: "hermes",
        config: ".hermes/config.yaml",
        kept: Kept::Yaml,
        add: Some(&["mcp", "add", NAME, "--command", "{exe}", "--args", "mcp"]),
        // It lists the server's tools and asks whether to enable them all.
        answer: Some("Y\n"),
        remove: Some(&["mcp", "remove", NAME]),
        after: "run /reload-mcp in Hermes, or start it again",
    },
];

/// The agent of integration `id`, when it takes MCP servers.
pub(super) fn host(id: &str) -> Option<&'static Host> {
    HOSTS.iter().find(|host| host.id == id)
}

impl Host {
    /// Its program and the PATH to run it with, when it is installed.
    fn installed(&self) -> Result<(std::path::PathBuf, String), String> {
        let path = login_path().ok_or("the login shell did not say its PATH")?;
        let program = which(self.program, &path)
            .ok_or_else(|| format!("not installed (no `{}` on the PATH)", self.program))?;
        Ok((program, path))
    }

    pub(super) fn state(&self, home: &Path, exe: &Path) -> State {
        if let Err(why) = self.installed() {
            return State::Unavailable(why);
        }
        let text = std::fs::read_to_string(home.join(self.config)).unwrap_or_default();
        match self.entry(&text) {
            Some(command) if command == exe.display().to_string() => {
                State::On(format!("in ~/{}", self.config))
            }
            Some(_) => State::Off("another program is named there".into()),
            None => State::Off("not added".into()),
        }
    }

    /// The command its configuration runs for the server, if it has one.
    fn entry(&self, text: &str) -> Option<String> {
        match self.kept {
            Kept::Json(at) => at_path(&serde_json::from_str(text).ok()?, at)?
                .as_str()
                .map(String::from),
            Kept::JsonLine(at) => at_path(&serde_json::from_str(text).ok()?, at)?
                .get(0)?
                .as_str()
                .map(String::from),
            Kept::Toml => toml_command(text),
            Kept::Yaml => yaml_command(text),
        }
    }

    pub(super) fn turn(&self, home: &Path, exe: &Path, on: bool) -> Result<String, String> {
        let (program, path) = self.installed()?;
        let exe_text = exe.display().to_string();
        let asked = match on {
            true => self.add,
            false => self.remove,
        };
        let Some(args) = asked else {
            return self.write(home, exe, on);
        };
        // One that names another program goes first: `add` keeps an entry it has.
        let had = std::fs::read_to_string(home.join(self.config)).unwrap_or_default();
        if on
            && self.entry(&had).is_some()
            && let Some(remove) = self.remove
        {
            let _ = run_with(&program, remove, &path, MOST, None);
        }
        let args: Vec<&str> = (args.iter())
            .map(|arg| {
                if *arg == "{exe}" {
                    exe_text.as_str()
                } else {
                    arg
                }
            })
            .collect();
        let ran = run_with(&program, &args, &path, MOST, self.answer.filter(|_| on))?;
        if !ran.ok {
            return Err(ran.out.trim().to_string());
        }
        Ok(match on {
            true => format!("added: {}", self.after),
            false => "taken out".into(),
        })
    }

    /// The server put in or taken out of the agent's JSON, the rest as it was (OpenCode's).
    fn write(&self, home: &Path, exe: &Path, on: bool) -> Result<String, String> {
        let file = home.join(self.config);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let written = opencode(&text, exe, on)?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
        }
        std::fs::write(&file, written).map_err(|why| format!("{}: {why}", file.display()))?;
        Ok(match on {
            true => format!("added: {}", self.after),
            false => "taken out".into(),
        })
    }
}

/// The value at `path` in `config`.
fn at_path<'a>(config: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(config, |value, key| value.get(*key))
}

/// The server's entry, as every MCP host's JSON has it.
pub(super) fn snippet(exe: &Path) -> Value {
    json!({"mcpServers": {NAME: {"command": exe.display().to_string(), "args": ["mcp"]}}})
}

/// OpenCode's configuration with the server put in (a local one, its command line) or taken
/// out, the rest as it was.
fn opencode(text: &str, exe: &Path, on: bool) -> Result<String, String> {
    let mut config: Value = match text.trim() {
        "" => json!({}),
        text => serde_json::from_str(text)
            .map_err(|why| format!("the OpenCode configuration is no JSON: {why}"))?,
    };
    let servers = config
        .as_object_mut()
        .ok_or("the OpenCode configuration is no JSON object")?
        .entry("mcp")
        .or_insert_with(|| json!({}));
    let servers = servers.as_object_mut().ok_or("its `mcp` is no object")?;
    match on {
        true => {
            let line = json!([exe.display().to_string(), "mcp"]);
            servers.insert(
                NAME.into(),
                json!({"type": "local", "command": line, "enabled": true}),
            );
        }
        false => {
            servers.remove(NAME);
        }
    }
    serde_json::to_string_pretty(&config)
        .map(|text| text + "\n")
        .map_err(|why| why.to_string())
}

/// The heading of the server's table in Codex's TOML.
const TOML_TABLE: &str = "[mcp_servers.llx-desk]";

/// `command` in the server's TOML table (from its heading to the next table).
fn toml_command(text: &str) -> Option<String> {
    let (_, rest) = text.split_once(TOML_TABLE)?;
    let table = rest.split_once("\n[").map_or(rest, |(table, _)| table);
    let line = table
        .lines()
        .find(|line| line.trim_start().starts_with("command"))?;
    let value = line.split_once('=')?.1.trim();
    serde_json::from_str::<String>(value).ok()
}

/// `command:` under the server's key in YAML: the first one more indented than the key, before
/// the next key as indented as it.
fn yaml_command(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let indent = |line: &str| line.len() - line.trim_start().len();
    let key = lines.find(|line| line.trim() == format!("{NAME}:"))?;
    let depth = indent(key);
    let value = lines
        .take_while(|line| line.trim().is_empty() || indent(line) > depth)
        .find_map(|line| line.trim().strip_prefix("command:"))?
        .trim();
    let unquoted = value.trim_matches(|c| c == '"' || c == '\'');
    (!unquoted.is_empty()).then(|| unquoted.to_string())
}

#[cfg(test)]
#[path = "mcp_hosts_tests.rs"]
mod tests;
