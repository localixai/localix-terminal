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
    /// How a command of its starts on a first prompt and stays to talk: the words before the
    /// prompt (none: the prompt is its first word). A command not here takes none.
    pub seeds: &'static [(&'static str, &'static str)],
}

/// Brand marks (but Hermes's): see NOTICE.md.
pub(super) const AGENTS: [Agent; 6] = [
    Agent {
        id: "claude",
        name: "Claude Code",
        commands: &["claude"],
        mark: include_str!("marks/claude.svg"),
        seeds: &[("claude", "")],
    },
    Agent {
        id: "codex",
        name: "Codex",
        commands: &["codex"],
        mark: include_str!("marks/codex.svg"),
        seeds: &[("codex", "")],
    },
    Agent {
        id: "gemini",
        name: "Gemini CLI",
        commands: &["gemini"],
        mark: include_str!("marks/gemini.svg"),
        seeds: &[("gemini", "-i")],
    },
    Agent {
        id: "copilot",
        name: "GitHub Copilot",
        commands: &["copilot"],
        mark: include_str!("marks/copilot.svg"),
        seeds: &[("copilot", "-i")],
    },
    Agent {
        id: "opencode",
        name: "OpenCode",
        commands: &["opencode"],
        mark: include_str!("marks/opencode.svg"),
        seeds: &[("opencode", "--prompt")],
    },
    Agent {
        id: "hermes",
        name: "Hermes",
        commands: &["hermes"],
        mark: include_str!("marks/hermes.svg"),
        seeds: &[("hermes", "chat -q")],
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

    /// The command line of `command` (one of its own) started on `prompt`, quoted for `sh`:
    /// none where that command takes no first prompt.
    pub(super) fn seeded(&self, command: &str, prompt: &str) -> Option<String> {
        let (_, before) = self.seeds.iter().find(|(seeded, _)| *seeded == command)?;
        let quoted = format!("'{}'", prompt.replace('\'', r"'\''"));
        Some(
            [command, before, &quoted]
                .into_iter()
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

/// Agent `id` started on `prompt`, where it is installed and takes a first prompt: its command
/// line, for a terminal of the person's. Said why not, as the person reads it.
pub fn started_on(id: &str, prompt: &str) -> Result<String, String> {
    let agent = (AGENTS.iter())
        .find(|agent| agent.id == id)
        .ok_or_else(|| format!("there is no agent `{id}`"))?;
    let command =
        (agent.command_on(&path())).ok_or_else(|| format!("{} is not installed", agent.name))?;
    (agent.seeded(command, prompt))
        .ok_or_else(|| format!("{} (`{command}`) takes no first prompt", agent.name))
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
    /// The agents found on it, as [`known`] says them.
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

/// The agents installed (their command on [`path`]): a JSON array of their place in
/// [`AGENTS`], id and name, for a page the desk paints (the board's agents to run a card with).
/// Looked for again as it is asked, so one installed since shows.
pub fn known() -> String {
    let mut known = crate::lock(&KNOWN);
    let path = (known.as_ref()).map_or_else(process_path, |known| known.path.clone());
    let now = Known::on(path);
    let installed = now.installed.clone();
    *known = Some(now);
    installed
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
