//! The coding agents the person can start from "+", each in a terminal of the project: its name,
//! its command, and its mark for the menu. Offered where its command is on the `PATH`, and shown
//! greyed where it is not. The desk only starts the program, as a terminal the person opened:
//! it knows nothing of the agent, and the agent works in it as anyone would.

use std::path::Path;

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
    pub(super) fn command_on(&self, path: &std::ffi::OsStr) -> Option<&'static str> {
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
