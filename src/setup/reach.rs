//! What setup knows of the person's coding agents, lent to the Store (`crate::catalog`), so
//! that it knows each thing once: where an agent keeps its configuration, how that file is read
//! (JSON with comments, a TOML table's values) and written whole, and how one of its programs is
//! found on the login shell's PATH and run within a deadline. The Store passes the home and the
//! variables it was given, so its tests name a temporary home and never the person's own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

pub(crate) use super::shell::Ran;
use super::{claude, mcp_hosts, shell};

/// The configuration file agent `id` keeps its MCP servers in (`codex`, `gemini`, `copilot`,
/// `opencode`, `hermes`), for the home `home` and the variables `var` reads; none for another.
pub(crate) fn config_file(
    id: &str,
    home: &Path,
    var: impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    mcp_hosts::config_of(id, home, var)
}

/// Claude Code's own folder (`CLAUDE_CONFIG_DIR`, else `~/.claude`): its skills, plugins and
/// settings.
pub(crate) fn claude_dir(home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
    claude::dir(home, var)
}

/// The file Claude Code keeps its user MCP servers in: `.claude.json` in the folder
/// `CLAUDE_CONFIG_DIR` names, else in the home directory.
pub(crate) fn claude_json(home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
    var("CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.trim().is_empty())
        .map_or_else(|| home.to_path_buf(), PathBuf::from)
        .join(".claude.json")
}

/// `text` as JSON, with or without the comments some agents keep in it.
pub(crate) fn json(text: &str) -> Option<Value> {
    mcp_hosts::json(text)
}

/// Key `key` of table `table` (`mcp_servers.weather`, `plugins."pdf@acme"`) in a TOML text, as
/// an agent's own command writes it: a string, a list of strings, or a flag.
pub(crate) fn toml_value(text: &str, table: &str, key: &str) -> Option<Value> {
    mcp_hosts::toml_value(text, table, key)
}

/// Why a program that failed did: what it said, or when it said nothing, `what` it means.
pub(crate) fn failure(ran: &Ran, what: &str) -> String {
    shell::failure(ran, what)
}

/// `text` written over `file` at once, its mode kept, a link followed; its folder made first.
pub(crate) fn write_over(file: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    }
    mcp_hosts::write_over(file, text).map_err(|why| format!("{}: {why}", file.display()))
}

/// OpenCode's configuration `text` with its MCP server `name` set to `entry`, or taken out when
/// there is none: the one writer of OpenCode's file, the app's own entry's too.
pub(crate) fn opencode_server(
    text: &str,
    name: &str,
    entry: Option<Value>,
) -> Result<String, String> {
    mcp_hosts::opencode_server(text, name, entry)
}

/// `file` as the person reads it: under their home, from `~`.
pub(crate) fn shown(file: &Path, home: &Path) -> String {
    mcp_hosts::shown(file, home)
}

/// The program `name` on `path` (a login shell's PATH), without starting anything.
pub(crate) fn which(name: &str, path: &str) -> Option<PathBuf> {
    shell::which(name, path)
}

/// The variables of the person's login shell that say where their agents keep things
/// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`…), each when it is set. It may ask the shell, which takes
/// seconds: never on the window's thread.
pub(crate) fn login_vars() -> BTreeMap<String, String> {
    (shell::HOMES.iter())
        .filter_map(|name| Some((name.to_string(), shell::login_var(name)?)))
        .collect()
}

/// `program` run with `args`, its PATH `path` and the variables `vars`, stopped past `most`:
/// what it said (its output, then its errors) and whether it ended well.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    path: &str,
    vars: &BTreeMap<String, String>,
    most: Duration,
) -> Result<Ran, String> {
    let vars: Vec<(String, String)> = (vars.iter())
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    shell::run_given(program, args, path, &vars, most, None)
}
