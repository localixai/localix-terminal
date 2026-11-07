//! The coding agents the person can start from "+", each in a terminal of the project: its name,
//! its command, and its mark for the menu. Offered where its command is on the `PATH`, and shown
//! greyed where it is not. The desk only starts the program, as a terminal the person opened:
//! it knows nothing of the agent, and the agent works in it as anyone would.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::Mutex;

use tao::event_loop::EventLoopProxy;

use super::host::Cmd;

/// A coding agent's command line program.
pub(super) struct Agent {
    /// How the window's pages name it (`claude`).
    pub id: &'static str,
    pub name: &'static str,
    /// What the terminal runs: the first one of these on the `PATH`.
    pub commands: &'static [&'static str],
    /// Its mark, an SVG drawn in one colour: the menu tints it as its text.
    pub mark: &'static str,
}

/// Brand marks (but Hermes's): see NOTICE.md.
pub(super) const AGENTS: [Agent; 6] = [
    Agent {
        id: "claude",
        name: "Claude Code",
        commands: &["claude"],
        mark: include_str!("marks/claude.svg"),
    },
    Agent {
        id: "codex",
        name: "Codex",
        commands: &["codex"],
        mark: include_str!("marks/codex.svg"),
    },
    Agent {
        id: "gemini",
        name: "Gemini CLI",
        commands: &["gemini"],
        mark: include_str!("marks/gemini.svg"),
    },
    Agent {
        id: "copilot",
        name: "GitHub Copilot",
        commands: &["copilot", "ghcs"],
        mark: include_str!("marks/copilot.svg"),
    },
    Agent {
        id: "opencode",
        name: "OpenCode",
        commands: &["opencode"],
        mark: include_str!("marks/opencode.svg"),
    },
    Agent {
        id: "hermes",
        name: "Hermes",
        commands: &["hermes", "hermes-agent"],
        mark: include_str!("marks/hermes.svg"),
    },
];

impl Agent {
    /// The command to run, found on `path` (a `PATH`): none where the agent is not installed.
    pub(super) fn command_on(&self, path: &OsStr) -> Option<&'static str> {
        let dirs: Vec<_> = std::env::split_paths(path).collect();
        self.commands
            .iter()
            .copied()
            .find(|command| dirs.iter().any(|dir| runnable(&dir.join(command))))
    }
}

/// The agents' marks for a page, as a JSON object of SVG by id, each drawn in the text's colour.
pub(super) fn marks() -> String {
    let marks: serde_json::Map<String, serde_json::Value> = AGENTS
        .iter()
        .map(|agent| {
            let svg = agent.mark.trim().replace("\"#000\"", "\"currentColor\"");
            (agent.id.to_string(), svg.into())
        })
        .collect();
    serde_json::Value::Object(marks).to_string()
}

/// What the agents are looked for on, and what was found there when the pages last asked.
struct Known {
    /// The `PATH`: the process's, then the person's login shell's once it has said (an app
    /// started from the Finder or the Dock has only the system's).
    path: OsString,
    /// The agents found on it, as [`installed`] says them.
    installed: String,
}

static KNOWN: Mutex<Option<Known>> = Mutex::new(None);

/// The `PATH` the agents are looked for on.
pub(super) fn path() -> OsString {
    let mut known = crate::lock(&KNOWN);
    known
        .get_or_insert_with(|| Known::on(process_path()))
        .path
        .clone()
}

fn process_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

impl Known {
    fn on(path: OsString) -> Self {
        let found: Vec<serde_json::Value> = (AGENTS.iter().enumerate())
            .filter(|(_, agent)| agent.command_on(&path).is_some())
            .map(|(at, agent)| serde_json::json!({"at": at, "id": agent.id, "name": agent.name}))
            .collect();
        Self {
            path,
            installed: serde_json::Value::Array(found).to_string(),
        }
    }
}

/// `login`, the PATH of the person's login shell, then what the process has beyond it.
fn merged(login: &str, process: &OsStr) -> OsString {
    let mut dirs: Vec<std::path::PathBuf> = std::env::split_paths(login).collect();
    for dir in std::env::split_paths(process) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| process.to_os_string())
}

/// Ask the person's login shell for its `PATH`, on a thread (it may take seconds, and runs their
/// profile): the agents are looked for on it from then on, and the window told to draw again.
pub(super) fn learn(proxy: &EventLoopProxy<Cmd>) {
    let proxy = proxy.clone();
    let asked = std::thread::Builder::new()
        .name("desk-agents-path".into())
        .spawn(move || {
            let Some(login) = crate::setup::login_path() else {
                return;
            };
            *crate::lock(&KNOWN) = Some(Known::on(merged(&login, &process_path())));
            let _ = proxy.send_event(Cmd::Strip);
        });
    if let Err(error) = asked {
        super::debug(|| format!("the login PATH was not asked for: {error}"));
    }
}

/// The agents installed (their command on [`path`]), for a page being built: a JSON array of
/// their place in [`AGENTS`], id and name. Looked for again as it is asked.
pub(super) fn installed() -> String {
    let mut known = crate::lock(&KNOWN);
    let path = (known.as_ref()).map_or_else(process_path, |known| known.path.clone());
    let now = Known::on(path);
    let installed = now.installed.clone();
    *known = Some(now);
    installed
}

/// The call that gives a page that is up the agents installed, as it last looked: said on a
/// refresh, so a page built before the login shell answered learns it too.
pub(super) fn script() -> String {
    let mut known = crate::lock(&KNOWN);
    let known = known.get_or_insert_with(|| Known::on(process_path()));
    format!("agentsInstalled({})", known.installed)
}

/// The agent a terminal's command line starts, read as a shell would start it: past variables
/// set in front (`FOO=1`) and the programs that run another (`env`, `exec`, `nohup`, `time`,
/// `command`), the first word's file name, whatever directory it is named in.
pub(super) fn of_command(line: &str) -> Option<&'static Agent> {
    const RUNNERS: [&str; 6] = ["env", "exec", "command", "nohup", "time", "builtin"];
    let program = line
        .split_whitespace()
        .find(|word| {
            let file = word.rsplit('/').next().unwrap_or(word);
            !word.contains('=') && !word.starts_with('-') && !RUNNERS.contains(&file)
        })?
        .rsplit('/')
        .next()?;
    AGENTS
        .iter()
        .find(|agent| agent.commands.contains(&program))
}

fn runnable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(file)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
