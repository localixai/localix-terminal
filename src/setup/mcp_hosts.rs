//! The window tool in the MCP hosts the person has (Cursor, Codex): an `llx-desk` server in each
//! one's own configuration, run as `<this program> mcp`. Any other host is given the same entry
//! to paste (`snippet`).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::State;

/// The name of the server in a host's configuration.
const NAME: &str = "llx-desk";

/// An MCP host, by how its configuration is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Host {
    /// `~/.cursor/mcp.json`: `mcpServers` in JSON.
    Cursor,
    /// `~/.codex/config.toml`: an `[mcp_servers.<name>]` table.
    Codex,
}

impl Host {
    fn file(self, home: &Path) -> PathBuf {
        match self {
            Self::Cursor => home.join(".cursor/mcp.json"),
            Self::Codex => home.join(".codex/config.toml"),
        }
    }

    /// The host is there: its directory is.
    fn present(self, home: &Path) -> bool {
        self.file(home).parent().is_some_and(Path::is_dir)
    }

    pub(super) fn state(self, home: &Path, exe: &Path) -> State {
        if !self.present(home) {
            return State::Unavailable(format!("not installed (no {})", parent(&self.file(home))));
        }
        let text = std::fs::read_to_string(self.file(home)).unwrap_or_default();
        match self.entry(&text) {
            Some(command) if command == exe.display().to_string() => {
                State::On(format!("in {}", self.file(home).display()))
            }
            Some(_) => State::Off("another program is named there".into()),
            None => State::Off("not added".into()),
        }
    }

    /// The command the configuration runs for the server, if it has one.
    fn entry(self, text: &str) -> Option<String> {
        match self {
            Self::Cursor => {
                let config: Value = serde_json::from_str(text).ok()?;
                config["mcpServers"][NAME]["command"]
                    .as_str()
                    .map(String::from)
            }
            Self::Codex => {
                let table = table_of(text)?;
                let line = table
                    .lines()
                    .find(|line| line.trim_start().starts_with("command"))?;
                let value = line.split_once('=')?.1.trim();
                serde_json::from_str::<String>(value).ok()
            }
        }
    }

    pub(super) fn turn(self, home: &Path, exe: &Path, on: bool) -> Result<String, String> {
        let file = self.file(home);
        if !self.present(home) {
            return Err(format!("not installed (no {})", parent(&file)));
        }
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let written = match self {
            Self::Cursor => cursor(&text, exe, on)?,
            Self::Codex => codex(&text, exe, on),
        };
        std::fs::write(&file, written).map_err(|why| format!("{}: {why}", file.display()))?;
        Ok(match on {
            true => format!(
                "added to {}: restart it to use the window tool",
                file.display()
            ),
            false => format!("taken out of {}", file.display()),
        })
    }
}

fn parent(file: &Path) -> String {
    file.parent()
        .map_or_else(String::new, |dir| dir.display().to_string())
}

/// The server's entry, as every MCP host's JSON has it.
pub(super) fn snippet(exe: &Path) -> Value {
    json!({"mcpServers": {NAME: {"command": exe.display().to_string(), "args": ["mcp"]}}})
}

/// Cursor's configuration with the server added or taken out, the rest as it was.
fn cursor(text: &str, exe: &Path, on: bool) -> Result<String, String> {
    let mut config: Value = match text.trim() {
        "" => json!({}),
        text => serde_json::from_str(text)
            .map_err(|why| format!("~/.cursor/mcp.json is no JSON: {why}"))?,
    };
    let servers = config
        .as_object_mut()
        .ok_or("~/.cursor/mcp.json is no JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or("its mcpServers is no object")?;
    match on {
        true => {
            servers.insert(NAME.into(), snippet(exe)["mcpServers"][NAME].clone());
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
const CODEX_TABLE: &str = "[mcp_servers.llx-desk]";

/// The server's table in `text` (from its heading to the next table), if it has one.
fn table_of(text: &str) -> Option<&str> {
    let (_, rest) = text.split_once(CODEX_TABLE)?;
    Some(match rest.split_once("\n[") {
        // Up to its last line's end: the next table's `[` stays.
        Some((table, _)) => rest.get(..=table.len()).unwrap_or(table),
        None => rest,
    })
}

/// Codex's configuration with the server's table put in or taken out, the rest as it was.
fn codex(text: &str, exe: &Path, on: bool) -> String {
    let mut kept = text.to_string();
    if let Some(start) = kept.find(CODEX_TABLE) {
        let table = table_of(&kept).map_or(0, str::len);
        kept.replace_range(start..start + CODEX_TABLE.len() + table, "");
    }
    if !on {
        return kept;
    }
    let command = serde_json::to_string(&exe.display().to_string()).unwrap_or_default();
    let gap = match kept.is_empty() || kept.ends_with("\n\n") {
        true => "",
        false if kept.ends_with('\n') => "\n",
        false => "\n\n",
    };
    format!("{kept}{gap}{CODEX_TABLE}\ncommand = {command}\nargs = [\"mcp\"]\n")
}

#[cfg(test)]
mod tests {
    use super::{Host, codex, cursor};
    use std::path::Path;

    #[test]
    fn cursor_keeps_the_other_servers_and_takes_out_only_its_own() {
        let exe = Path::new("/A/llx-desk");
        let had = r#"{"mcpServers": {"other": {"command": "x"}}, "theme": 1}"#;
        let on = cursor(had, exe, true).unwrap();
        assert_eq!(Host::Cursor.entry(&on).as_deref(), Some("/A/llx-desk"));
        assert!(on.contains("\"other\"") && on.contains("\"theme\""));
        let off = cursor(&on, exe, false).unwrap();
        assert_eq!(Host::Cursor.entry(&off), None);
        assert!(off.contains("\"other\""));
        assert!(cursor("", exe, true).unwrap().contains("llx-desk"));
    }

    #[test]
    fn codex_gets_its_table_once_and_loses_only_it() {
        let exe = Path::new("/A/llx-desk");
        let had = "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let on = codex(had, exe, true);
        assert_eq!(Host::Codex.entry(&on).as_deref(), Some("/A/llx-desk"));
        assert_eq!(
            codex(&on, exe, true)
                .matches("[mcp_servers.llx-desk]")
                .count(),
            1
        );
        let off = codex(&on, exe, false);
        assert_eq!(Host::Codex.entry(&off), None);
        assert!(off.contains("[mcp_servers.other]") && off.contains("model = \"o3\""));
        // A table in the middle goes without what follows it.
        let middle = "[mcp_servers.llx-desk]\ncommand = \"/old\"\nargs = [\"mcp\"]\n[mcp_servers.other]\ncommand = \"x\"\n";
        let moved = codex(middle, exe, true);
        assert!(
            moved.contains("[mcp_servers.other]\ncommand = \"x\""),
            "{moved}"
        );
        assert_eq!(Host::Codex.entry(&moved).as_deref(), Some("/A/llx-desk"));
    }
}
