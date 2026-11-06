//! What the app puts in place for the person's tools, so that the app is all there is to
//! install: the `llx-desk` command on their PATH, its plugin in Claude Code, its window tool in
//! the other coding agents of the window's (agents.rs) that take MCP servers; any other MCP host
//! is given the entry to paste. Each is an integration with a state and an act, reached two ways:
//! Settings → Integrations in the window and `llx-desk setup` in a terminal.
//!
//! The app puts every one in place itself, for whatever agent is installed, each time it opens
//! (`opened`, `auto`): nobody has to set anything up. What the person takes out stays out
//! (`declined`, kept in the store) until they put it back.

mod claude;
mod command;
mod mcp_hosts;
mod shell;

use std::path::PathBuf;

use serde_json::{Value, json};

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

/// What the window offers where an agent runs while its integration is off: put it in, and what
/// to do after.
const fn offer(says: &'static str, take: &'static str, after: &'static str) -> Offered {
    Offered { says, take, after }
}

/// Every integration, in the order they are listed: the command, then the agents in the
/// window's order (agents.rs).
pub const INTEGRATIONS: [Integration; 7] = [
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
        offer: Some(offer(
            "Add the LLX Desk plugin to Claude Code: pages, the board and your comments, in this window.",
            "Add Plugin",
            "Plugin added. Run /reload-plugins in Claude Code to use it.",
        )),
    },
    Integration {
        id: "codex",
        title: "Codex",
        gives: "The window tool as an MCP server.",
        agent: Some("codex"),
        offer: Some(offer(
            "Add LLX Desk's window tool to Codex: pages and what you point at, in this window.",
            "Add to Codex",
            "Added to Codex. Start Codex again to use it.",
        )),
    },
    Integration {
        id: "gemini",
        title: "Gemini CLI",
        gives: "The window tool as an MCP server.",
        agent: Some("gemini"),
        offer: Some(offer(
            "Add LLX Desk's window tool to Gemini CLI: pages and what you point at, in this window.",
            "Add to Gemini",
            "Added to Gemini CLI. Start it again to use it.",
        )),
    },
    Integration {
        id: "copilot",
        title: "GitHub Copilot",
        gives: "The window tool as an MCP server.",
        agent: Some("copilot"),
        offer: Some(offer(
            "Add LLX Desk's window tool to Copilot: pages and what you point at, in this window.",
            "Add to Copilot",
            "Added to Copilot. Start it again to use it.",
        )),
    },
    Integration {
        id: "opencode",
        title: "OpenCode",
        gives: "The window tool as an MCP server.",
        agent: Some("opencode"),
        offer: Some(offer(
            "Add LLX Desk's window tool to OpenCode: pages and what you point at, in this window.",
            "Add to OpenCode",
            "Added to OpenCode. Start it again to use it.",
        )),
    },
    Integration {
        id: "hermes",
        title: "Hermes",
        gives: "The window tool as an MCP server.",
        agent: Some("hermes"),
        offer: Some(offer(
            "Add LLX Desk's window tool to Hermes: pages and what you point at, in this window.",
            "Add to Hermes",
            "Added to Hermes. Run /reload-mcp in it to use it.",
        )),
    },
];

/// The integration for agent `agent` (its program's name, `claude`), when there is one to offer.
pub fn for_agent(agent: &str) -> Option<&'static Integration> {
    (INTEGRATIONS.iter())
        .find(|integration| integration.agent == Some(agent) && integration.offer.is_some())
}

/// Whether the window offers integration `id` where its agent runs: it is off, could be on, and
/// the person did not take it out. It runs the person's programs: not for the window's thread.
pub fn offered(id: &str) -> bool {
    matches!(state(id), State::Off(_)) && !declined(id)
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
    match (id, mcp_hosts::host(id)) {
        ("command", _) => command::state(&exe, &home),
        ("claude-code", _) => claude::state(),
        (_, Some(host)) => host.state(&home, &exe),
        (other, None) => State::Unavailable(format!("no integration `{other}`")),
    }
}

/// The person puts integration `id` in place (`on`) or takes it out: done, and remembered, so the
/// app never puts back what they took out. What was done, in words.
pub fn turn(id: &str, on: bool) -> Result<String, String> {
    let done = put(id, on)?;
    remember(id, !on);
    Ok(done)
}

/// Integration `id` put in place (`on`) or taken out; what was done, in words.
fn put(id: &str, on: bool) -> Result<String, String> {
    let (exe, home) = (program()?, home()?);
    match (id, on, mcp_hosts::host(id)) {
        ("command", true, _) => {
            let linked = command::put_on_path(&exe, &home)?;
            // Elsewhere the program is the app, and the person's applications list it.
            #[cfg(target_os = "linux")]
            let linked = format!("{linked}; {}", desktop_entry(&exe, &home)?);
            Ok(linked)
        }
        ("command", false, _) => command::take_off_path(&exe, &home),
        ("claude-code", true, _) => claude::turn_on(),
        ("claude-code", false, _) => claude::turn_off(),
        (_, on, Some(host)) => host.turn(&home, &exe, on),
        (other, _, None) => Err(format!("no integration `{other}`")),
    }
}

/// The store's key of "the person took integration `id` out".
fn declined_key(id: &str) -> String {
    format!("integration.declined.{id}")
}

/// Whether the person took integration `id` out: the app leaves it out.
pub fn declined(id: &str) -> bool {
    let Ok(store) = crate::store::Store::open(&crate::paths::database()) else {
        return false;
    };
    let key = declined_key(id);
    store
        .read(|db| {
            use rusqlite::OptionalExtension as _;
            db.query_row("SELECT 1 FROM state WHERE key = ?1", [&key], |_| Ok(()))
                .optional()
                .map(|found| found.is_some())
                .map_err(|why| why.to_string())
        })
        .unwrap_or(false)
}

/// The person took integration `id` out (`out`), or put it back.
fn remember(id: &str, out: bool) {
    let written = crate::store::Store::open(&crate::paths::database()).and_then(|store| {
        let key = declined_key(id);
        store.write(|tx| {
            let done = match out {
                true => tx.execute(
                    "INSERT INTO state (key, value) VALUES (?1, '1')
                     ON CONFLICT (key) DO UPDATE SET value = '1'",
                    [&key],
                ),
                false => tx.execute("DELETE FROM state WHERE key = ?1", [&key]),
            };
            done.map(drop).map_err(|why| why.to_string())
        })
    });
    if let Err(why) = written {
        log::warn!("setup: what the person did to {id} is not remembered: {why}");
    }
}

/// Every integration with its state, as Settings shows them; each read on a thread of its own
/// (an agent's program may take a moment to answer).
pub fn states() -> Value {
    let read: Vec<State> = std::thread::scope(|scope| {
        // Every one started before any is waited for.
        let mut reading = Vec::with_capacity(INTEGRATIONS.len());
        for integration in &INTEGRATIONS {
            let read = std::thread::Builder::new()
                .name("desk-setup-state".into())
                .spawn_scoped(scope, || state(integration.id));
            reading.push(read);
        }
        (reading.into_iter())
            .map(|read| {
                (read.ok().and_then(|read| read.join().ok()))
                    .unwrap_or_else(|| State::Unavailable("it could not be read".into()))
            })
            .collect()
    });
    let all: Vec<Value> = (INTEGRATIONS.iter().zip(read))
        .map(|(integration, state)| {
            json!({
                "id": integration.id,
                "title": integration.title,
                "gives": integration.gives,
                "state": state.word(),
                "note": state.note(),
                // Taken out by the person: the app leaves it out, and offers it nowhere.
                "declined": matches!(state, State::Off(_)) && declined(integration.id),
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

/// The app opened: the command linked (unless the person took it out). What was done, in words,
/// for the app's log.
pub fn opened() -> Vec<String> {
    if declined("command") {
        return vec!["the command stays off the PATH, as the person left it".into()];
    }
    match put("command", true) {
        Ok(done) => vec![done],
        Err(why) => vec![format!("the command was not linked: {why}")],
    }
}

/// Every agent's integration put in place, for each agent installed, but what the person took
/// out: one not there, of an older app (the plugin), or naming a program that moved. It runs
/// their programs: on a thread of its own. What was done, in words, for the app's log.
pub fn auto() -> Vec<String> {
    let mut said = vec![];
    for integration in INTEGRATIONS.iter().filter(|known| known.id != "command") {
        let title = integration.title;
        if declined(integration.id) {
            said.push(format!("{title}: left out, as the person took it out"));
            continue;
        }
        match state(integration.id) {
            State::Off(why) => match put(integration.id, true) {
                Ok(done) => said.push(format!("{title} ({why}): {done}")),
                Err(failed) => said.push(format!("{title} ({why}) was not put in place: {failed}")),
            },
            State::On(_) | State::Unavailable(_) => {}
        }
    }
    said
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
