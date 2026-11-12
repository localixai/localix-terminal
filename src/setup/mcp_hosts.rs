//! The window tool in the coding agents that take MCP servers (Codex, Gemini CLI, GitHub Copilot,
//! OpenCode, Hermes): an `llxterm` server in each one's own configuration, run as
//! `<this program> mcp`. Each is put in and taken out with the agent's own commands
//! (`<agent> mcp add` / `remove`), where it has them for a local server; OpenCode has none, and its
//! JSON is written. Whether it is there is read from the agent's configuration file (read.rs),
//! where the agent itself looks for it (place.rs), which is quick and needs none of its
//! programs to start. Any other host is given the entry to paste (`snippet`).

mod place;
mod read;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::State;
use super::shell::{failure, find, login_var, run_with};
use place::Place;
pub(super) use place::shown;
use read::Kept;
pub(super) use read::{json, toml_value};

/// The name of the server in an agent's configuration.
pub(super) const NAME: &str = "llxterm";
/// How long an agent's `mcp` command may take (Hermes connects to the server to list its tools).
const MOST: std::time::Duration = std::time::Duration::from_secs(60);

/// A coding agent that takes MCP servers.
#[derive(Debug, Clone, Copy)]
pub(super) struct Host {
    /// Its integration's id, its agent's (agents.rs).
    pub(super) id: &'static str,
    /// Its program, on the person's PATH.
    program: &'static str,
    /// Where its configuration is, and how it keeps the server there.
    place: Place,
    kept: Kept,
    /// Its command that adds the server (`{exe}`: this program), none when it has none; what it
    /// is answered when it asks.
    add: Option<&'static [&'static str]>,
    answer: Option<&'static str>,
    /// Its command that takes the server out.
    remove: Option<&'static [&'static str]>,
    /// What its entry holds beside the command when this app added it (a path in its JSON):
    /// one without it is of an older app, and is added again.
    with: Option<&'static [&'static str]>,
    /// What to do after, in words.
    after: &'static str,
}

/// Every agent that takes MCP servers, as setup.rs lists them.
pub(super) const HOSTS: [Host; 5] = [
    Host {
        id: "codex",
        program: "codex",
        place: place(".codex", "CODEX_HOME", "config.toml", false),
        kept: Kept::Toml,
        add: Some(&["mcp", "add", NAME, "--", "{exe}", "mcp"]),
        answer: None,
        remove: Some(&["mcp", "remove", NAME]),
        with: None,
        after: "start Codex again to use it",
    },
    Host {
        id: "gemini",
        program: "gemini",
        place: place("", "GEMINI_CLI_HOME", ".gemini/settings.json", false),
        kept: Kept::Json(&["mcpServers", NAME, "command"]),
        add: Some(&["mcp", "add", "--scope", "user", NAME, "{exe}", "mcp"]),
        answer: None,
        remove: Some(&["mcp", "remove", "--scope", "user", NAME]),
        with: None,
        after: "run /mcp reload in Gemini CLI, or start it again",
    },
    Host {
        id: "copilot",
        program: "copilot",
        place: place(".copilot", "COPILOT_HOME", "mcp-config.json", false),
        kept: Kept::Json(&["mcpServers", NAME, "command"]),
        // Copilot gives a call 30 s unless told, and a terminal may be waited on that long.
        add: Some(&[
            "mcp",
            "add",
            "--timeout",
            "120000",
            NAME,
            "--",
            "{exe}",
            "mcp",
        ]),
        answer: None,
        remove: Some(&["mcp", "remove", NAME]),
        with: Some(&["mcpServers", NAME, "timeout"]),
        after: "run /mcp reload in Copilot, or start it again",
    },
    Host {
        id: "opencode",
        program: "opencode",
        place: place(
            ".config",
            "XDG_CONFIG_HOME",
            "opencode/opencode.json",
            false,
        ),
        kept: Kept::JsonLine(&["mcp", NAME, "command"]),
        add: None,
        answer: None,
        remove: None,
        with: None,
        after: "start OpenCode again to use it",
    },
    Host {
        id: "hermes",
        program: "hermes",
        place: place(".hermes", "HERMES_HOME", "config.yaml", true),
        kept: Kept::Yaml,
        add: Some(&["mcp", "add", NAME, "--command", "{exe}", "--args", "mcp"]),
        // It lists the server's tools and asks whether to enable them all.
        answer: Some("Y\n"),
        remove: Some(&["mcp", "remove", NAME]),
        with: None,
        after: "run /reload-mcp in Hermes, or start it again",
    },
];

const fn place(dir: &'static str, var: &'static str, file: &'static str, profiles: bool) -> Place {
    Place {
        dir,
        var,
        file,
        profiles,
    }
}

/// The agent of integration `id`, when it takes MCP servers.
pub(super) fn host(id: &str) -> Option<&'static Host> {
    HOSTS.iter().find(|host| host.id == id)
}

/// The configuration file of agent `id` (one that takes MCP servers), for the person whose home
/// is `home` and whose variables `var` reads.
pub(super) fn config_of(
    id: &str,
    home: &Path,
    var: impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    host(id).map(|host| host.place.config(home, var))
}

impl Host {
    /// Its configuration file: where the person's login shell says the agent keeps it.
    fn config(&self, home: &Path) -> PathBuf {
        self.place.config(home, login_var)
    }

    pub(super) fn state(&self, home: &Path, exe: &Path) -> State {
        if let Err(why) = find(self.program) {
            return State::Unavailable(why);
        }
        let file = self.config(home);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        match self.entry(&text) {
            Some(command) if command != exe.display().to_string() => {
                State::Off("another program is named there".into())
            }
            Some(_) if !self.whole(&text) => State::Off("added by an older app".into()),
            Some(_) => State::On(format!("in {}", shown(&file, home))),
            None => State::Off("not added".into()),
        }
    }

    /// The command its configuration runs for the server, if it has one.
    fn entry(&self, text: &str) -> Option<String> {
        self.kept.entry(text)
    }

    /// Whether its entry in `text` holds all this app adds with it.
    fn whole(&self, text: &str) -> bool {
        self.with.is_none_or(|at| read::has(text, at))
    }

    pub(super) fn turn(&self, home: &Path, exe: &Path, on: bool) -> Result<String, String> {
        let (program, path) = find(self.program)?;
        let exe_text = exe.display().to_string();
        let asked = match on {
            true => self.add,
            false => self.remove,
        };
        let Some(args) = asked else {
            return self.write(home, exe, on);
        };
        let had = std::fs::read_to_string(self.config(home)).unwrap_or_default();
        match self.entry(&had) {
            // There as it would be added: left alone, with what the person set beside it (an
            // agent's `add` writes the entry anew, without their timeouts and tool lists).
            Some(command) if on && command == exe_text && self.whole(&had) => {
                return Ok(format!("already added: {}", self.after));
            }
            // One that names another program goes first: `add` keeps an entry it has.
            Some(_) if on => {
                if let Some(remove) = self.remove {
                    let _ = run_with(&program, remove, &path, MOST, None);
                }
            }
            _ => {}
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
            return Err(failure(
                &ran,
                &format!("`{} mcp` failed and said nothing", self.program),
            ));
        }
        Ok(match on {
            true => format!("added: {}", self.after),
            false => "taken out".into(),
        })
    }

    /// The server put in or taken out of the agent's JSON, the rest as it was (OpenCode's).
    fn write(&self, home: &Path, exe: &Path, on: bool) -> Result<String, String> {
        let file = self.config(home);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let written = opencode(&text, exe, on)?;
        // Nothing to take out of a configuration that has no server: not even a file is made.
        if written != text {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
            }
            write_over(&file, &written).map_err(|why| format!("{}: {why}", file.display()))?;
        }
        Ok(match on {
            true => format!("added: {}", self.after),
            false => "taken out".into(),
        })
    }
}

/// `text` in `file` all at once: written beside it and renamed over it, so a failure half way
/// leaves the person's file as it was. A link (a dotfile manager's) is followed, so it stays one.
pub(super) fn write_over(file: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    let real = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let temp = real.with_extension(format!("llx{}", std::process::id()));
    // The file's own mode from the first byte (it may hold tokens), the owner's alone when it is
    // new; the temporary is made new, so a link left at its name is never written through.
    let meta = std::fs::metadata(&real).ok();
    let mode = (meta.as_ref()).map_or(0o600, |meta| meta.permissions().mode() & 0o7777);
    let _ = std::fs::remove_file(&temp);
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp)
        .and_then(|mut made| made.write_all(text.as_bytes()))
        .and_then(|()| {
            // The umask took bits off the mode it was made with.
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(mode))?;
            std::fs::rename(&temp, &real)
        });
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// The server's entry, as every MCP host's JSON has it.
pub(super) fn snippet(exe: &Path) -> Value {
    json!({"mcpServers": {NAME: {"command": exe.display().to_string(), "args": ["mcp"]}}})
}

/// OpenCode's configuration with the server put in (a local one, its command line) or taken
/// out, the rest as it was.
fn opencode(text: &str, exe: &Path, on: bool) -> Result<String, String> {
    let line = json!([exe.display().to_string(), "mcp"]);
    let entry = on.then(|| json!({"type": "local", "command": line, "enabled": true}));
    opencode_server(text, NAME, entry)
}

/// OpenCode's configuration with its MCP server `name` set to `entry`, or taken out when there is
/// none, the rest as it was. A configuration that has no such server to take out is given back
/// as it was, not written again.
pub(super) fn opencode_server(
    text: &str,
    name: &str,
    entry: Option<Value>,
) -> Result<String, String> {
    let mut config: Value = match text.trim() {
        "" => json!({}),
        text => serde_json::from_str(text)
            .map_err(|why| format!("the OpenCode configuration is no JSON: {why}"))?,
    };
    let config = config
        .as_object_mut()
        .ok_or("the OpenCode configuration is no JSON object")?;
    if let Some(entry) = entry {
        let servers = config.entry("mcp").or_insert_with(|| json!({}));
        let servers = servers.as_object_mut().ok_or("its `mcp` is no object")?;
        servers.insert(name.into(), entry);
    } else {
        let removed = (config.get_mut("mcp"))
            .and_then(Value::as_object_mut)
            .and_then(|servers| servers.remove(name));
        if removed.is_none() {
            return Ok(text.to_string());
        }
    }
    serde_json::to_string_pretty(&config)
        .map(|text| text + "\n")
        .map_err(|why| why.to_string())
}

#[cfg(test)]
#[path = "mcp_hosts_tests.rs"]
mod tests;
