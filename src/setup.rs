//! What the app puts in place for the person's tools, so that the app is all there is to
//! install: the `llx-desk` command on their PATH, its plugin in Claude Code, its window tool in
//! the MCP hosts they have (Cursor, Codex; any other is given the entry to paste). Each is an
//! integration with a state and an act, reached two ways: Settings → Integrations in the window
//! and `llx-desk setup` in a terminal. The app links the command itself when it opens and keeps
//! what the person turned on in step with its own version (`refresh`); the rest waits for them.

mod claude;
mod command;
mod mcp_hosts;
mod shell;

use std::path::PathBuf;

use serde_json::{Value, json};

use mcp_hosts::Host;

/// Where an integration is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// In place; what it is, in words.
    On(String),
    /// Not in place, and it could be; why not, in words.
    Off(String),
    /// It cannot be here; why, in words.
    Unavailable(String),
}

impl State {
    fn word(&self) -> &'static str {
        match self {
            Self::On(_) => "on",
            Self::Off(_) => "off",
            Self::Unavailable(_) => "unavailable",
        }
    }

    fn note(&self) -> &str {
        match self {
            Self::On(note) | Self::Off(note) | Self::Unavailable(note) => note,
        }
    }
}

/// An integration: its id, its name and what it gives; the agent it is for (its program, as a
/// terminal runs it), and what the window offers when that agent runs there while it is off.
pub struct Integration {
    pub id: &'static str,
    pub title: &'static str,
    pub gives: &'static str,
    pub agent: Option<&'static str>,
    pub offer: Option<Offered>,
}

/// An integration offered where its agent runs: the words, the button that turns it on, and what
/// to say once it is on.
pub struct Offered {
    pub says: &'static str,
    pub take: &'static str,
    pub after: &'static str,
}

/// Every integration, in the order they are listed.
pub const INTEGRATIONS: [Integration; 4] = [
    Integration {
        id: "command",
        title: "Command line",
        gives: "`llx-desk` in every terminal, for any agent that runs commands (on Linux, the app among your applications too).",
        agent: None,
        offer: None,
    },
    Integration {
        id: "claude-code",
        title: "Claude Code",
        gives: "The plugin: the window skill, the board, your comments told to Claude.",
        agent: Some("claude"),
        offer: Some(Offered {
            says: "Add the LLX Desk plugin to Claude Code: pages, the board and your comments, in this window.",
            take: "Add Plugin",
            after: "Plugin added. Run /reload-plugins in Claude Code to use it.",
        }),
    },
    Integration {
        id: "cursor",
        title: "Cursor",
        gives: "The window tool as an MCP server.",
        agent: None,
        offer: None,
    },
    Integration {
        id: "codex",
        title: "Codex",
        gives: "The window tool as an MCP server.",
        agent: Some("codex"),
        offer: Some(Offered {
            says: "Add LLX Desk's window tool to Codex: pages and what you point at, in this window.",
            take: "Add to Codex",
            after: "Added to Codex. Start Codex again to use it.",
        }),
    },
];

/// The integration for agent `agent` (its program's name, `claude`), when there is one to offer.
pub fn for_agent(agent: &str) -> Option<&'static Integration> {
    (INTEGRATIONS.iter())
        .find(|integration| integration.agent == Some(agent) && integration.offer.is_some())
}

/// Whether integration `id` is on, read as `state` reads it: not for the window's thread.
pub fn is_on(id: &str) -> bool {
    matches!(state(id), State::On(_))
}

/// This program, as the integrations name it: its path with links resolved (in the app, the
/// program of the bundle).
fn program() -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|why| format!("the program cannot find itself: {why}"))?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "no HOME".to_string())
}

/// Where integration `id` is. It may run the person's programs (`claude`) and their shell: not
/// for the window's thread.
pub fn state(id: &str) -> State {
    let (exe, home) = match (program(), home()) {
        (Ok(exe), Ok(home)) => (exe, home),
        (Err(why), _) | (_, Err(why)) => return State::Unavailable(why),
    };
    match id {
        "command" => command::state(&exe, &home),
        "claude-code" => claude::state(),
        "cursor" => Host::Cursor.state(&home, &exe),
        "codex" => Host::Codex.state(&home, &exe),
        other => State::Unavailable(format!("no integration `{other}`")),
    }
}

/// Integration `id` put in place (`on`) or taken out; what was done, in words.
pub fn turn(id: &str, on: bool) -> Result<String, String> {
    let (exe, home) = (program()?, home()?);
    match (id, on) {
        ("command", true) => {
            let linked = command::put_on_path(&exe, &home)?;
            // Elsewhere the program is the app, and the person's applications list it.
            #[cfg(target_os = "linux")]
            let linked = format!("{linked}; {}", desktop_entry(&exe, &home)?);
            Ok(linked)
        }
        ("command", false) => command::take_off_path(&exe, &home),
        ("claude-code", true) => claude::turn_on(),
        ("claude-code", false) => claude::turn_off(),
        ("cursor", on) => Host::Cursor.turn(&home, &exe, on),
        ("codex", on) => Host::Codex.turn(&home, &exe, on),
        (other, _) => Err(format!("no integration `{other}`")),
    }
}

/// Every integration with its state, as Settings shows them.
pub fn states() -> Value {
    let all: Vec<Value> = (INTEGRATIONS.iter())
        .map(|integration| {
            let state = state(integration.id);
            json!({
                "id": integration.id,
                "title": integration.title,
                "gives": integration.gives,
                "state": state.word(),
                "note": state.note(),
            })
        })
        .collect();
    Value::Array(all)
}

/// The MCP server's entry any host takes, to paste into one this app does not write itself.
pub fn mcp_entry() -> Result<String, String> {
    let entry = mcp_hosts::snippet(&program()?);
    serde_json::to_string_pretty(&entry).map_err(|why| why.to_string())
}

/// The app opened: the command linked (the one integration that is always on), and what the
/// person turned on before brought to this version. What was done, in words, for the app's log.
pub fn opened() -> Vec<String> {
    let mut said = vec![];
    match turn("command", true) {
        Ok(done) => said.push(done),
        Err(why) => said.push(format!("the command was not linked: {why}")),
    }
    said
}

/// What the person turned on, brought to this version: the plugin of an older app, an MCP
/// entry naming a program that moved. It runs their programs: on a thread of its own.
pub fn refresh() {
    claude::refresh();
    if let (Ok(exe), Ok(home)) = (program(), home()) {
        for host in [Host::Cursor, Host::Codex] {
            if matches!(host.state(&home, &exe), State::Off(why) if why.contains("another program"))
                && let Err(why) = host.turn(&home, &exe, true)
            {
                log::warn!("setup: an MCP entry was not updated: {why}");
            }
        }
    }
}

/// The app among the person's applications (Linux): a desktop entry that opens the window.
#[cfg(target_os = "linux")]
fn desktop_entry(exe: &std::path::Path, home: &std::path::Path) -> Result<String, String> {
    let dir = home.join(".local/share/applications");
    std::fs::create_dir_all(&dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    let file = dir.join("llx-desk.desktop");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=LLX Desk\nComment=Pages and terminals agents work in, and you watch\nExec=\"{}\" open\nTerminal=false\nCategories=Development;\n",
        exe.display()
    );
    std::fs::write(&file, entry).map_err(|why| format!("{}: {why}", file.display()))?;
    Ok(format!("{} opens the app", file.display()))
}
