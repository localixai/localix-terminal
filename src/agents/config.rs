//! Where the person's coding agents keep their configuration, and how a file of it is read and
//! written: under the home directory unless the person moved it with the agent's own variable
//! (`CODEX_HOME`); JSON with the comments some agents keep in it, a TOML table's values; a file
//! written whole, at once, so a failure half way leaves the person's file as it was.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Each agent that keeps MCP servers in a file of its own: its id, the folder under the home
/// directory, the variable that names another folder in place of that one, and the file under
/// the folder.
const PLACES: [(&str, &str, &str, &str); 4] = [
    ("codex", ".codex", "CODEX_HOME", "config.toml"),
    ("gemini", "", "GEMINI_CLI_HOME", ".gemini/settings.json"),
    ("copilot", ".copilot", "COPILOT_HOME", "mcp-config.json"),
    (
        "opencode",
        ".config",
        "XDG_CONFIG_HOME",
        "opencode/opencode.json",
    ),
];

/// The configuration file agent `id` keeps its MCP servers in (`codex`, `gemini`, `copilot`,
/// `opencode`), for the home `home` and the variables `var` reads; none for another.
pub(crate) fn config_file(
    id: &str,
    home: &Path,
    var: impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    let (_, dir, moved_by, file) = PLACES.iter().find(|(known, ..)| *known == id)?;
    Some(moved(home, &var, moved_by, dir).join(file))
}

/// The folder `dir` under `home`, or the one variable `name` names in its place.
fn moved(home: &Path, var: &impl Fn(&str) -> Option<String>, name: &str, dir: &str) -> PathBuf {
    var(name)
        .filter(|moved| !moved.trim().is_empty())
        .map_or_else(|| home.join(dir), PathBuf::from)
}

/// Claude Code's own folder (`CLAUDE_CONFIG_DIR`, else `~/.claude`): its skills, plugins and
/// settings.
pub(crate) fn claude_dir(home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
    moved(home, &var, "CLAUDE_CONFIG_DIR", ".claude")
}

/// The file Claude Code keeps its user MCP servers in: `.claude.json` in the folder
/// `CLAUDE_CONFIG_DIR` names, else in the home directory.
pub(crate) fn claude_json(home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
    moved(home, &var, "CLAUDE_CONFIG_DIR", "").join(".claude.json")
}

/// `file` as the person reads it: under their home, from `~`.
pub(crate) fn shown(file: &Path, home: &Path) -> String {
    match file.strip_prefix(home) {
        Ok(under) => format!("~/{}", under.display()),
        Err(_) => file.display().to_string(),
    }
}

/// `text` as JSON, with or without the comments some agents keep in it.
pub(crate) fn json(text: &str) -> Option<Value> {
    serde_json::from_str(text)
        .or_else(|_| serde_json::from_str(&uncommented(text)))
        .ok()
}

/// `text`, JSON with comments, without them: `//` to the end of its line and `/* */`, where
/// they are not inside a string. A line comment's newline stays, so the lines count the same.
fn uncommented(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text.chars().peekable();
    let mut in_string = false;
    while let Some(next) = rest.next() {
        match next {
            '\\' if in_string => {
                plain.push(next);
                plain.extend(rest.next());
            }
            '"' => {
                in_string = !in_string;
                plain.push(next);
            }
            '/' if !in_string && rest.peek() == Some(&'/') => {
                for skipped in rest.by_ref() {
                    if skipped == '\n' {
                        plain.push('\n');
                        break;
                    }
                }
            }
            '/' if !in_string && rest.peek() == Some(&'*') => {
                rest.next();
                let mut before = ' ';
                for skipped in rest.by_ref() {
                    if before == '*' && skipped == '/' {
                        break;
                    }
                    before = skipped;
                }
            }
            _ => plain.push(next),
        }
    }
    plain
}

/// Key `key` of table `table` (`mcp_servers.weather`, `plugins."pdf@acme"`) in a TOML text (from
/// the table's heading, `[table]`, to the next heading), when it is a string of either kind, a
/// list of basic strings or a flag on one line, as an agent's own command writes it. Whatever
/// follows the value (a comment) is not its.
pub(crate) fn toml_value(text: &str, table: &str, key: &str) -> Option<Value> {
    let heading = format!("[{table}]");
    let mut lines = text.lines().skip_while(|line| line.trim() != heading);
    lines.next()?;
    let value = lines
        .take_while(|line| !line.trim_start().starts_with('['))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim())
        })?;
    // A literal string, in single quotes, is its text as written; a basic one, a list of them
    // and a flag read as JSON reads them.
    if let Some(literal) = value.strip_prefix('\'') {
        return literal
            .split_once('\'')
            .map(|(text, _)| Value::String(text.to_string()));
    }
    serde_json::Deserializer::from_str(value)
        .into_iter::<Value>()
        .next()?
        .ok()
}

/// `text` written over `file` at once, its folder made first: written beside it and renamed over
/// it, so a failure half way leaves the person's file as it was. A link (a dotfile manager's) is
/// followed, so it stays one; the file keeps its mode.
pub(crate) fn write_over(file: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    }
    written(file, text).map_err(|why| format!("{}: {why}", file.display()))
}

fn written(file: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    let real = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let temp = real.with_extension(format!("llx{}", std::process::id()));
    // The file's own mode from the first byte (it may hold tokens), the owner's alone when it is
    // new; the temporary is made new, so a link left at its name is never written through.
    let meta = std::fs::metadata(&real).ok();
    let mode = (meta.as_ref()).map_or(0o600, |meta| meta.permissions().mode() & 0o7777);
    let _ = std::fs::remove_file(&temp);
    let done = std::fs::OpenOptions::new()
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
    if done.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    done
}

/// OpenCode's configuration `text` with its MCP server `name` set to `entry`, or taken out when
/// there is none, the rest as it was. A configuration that has no such server to take out is
/// given back as it was, not written again.
pub(crate) fn opencode_server(
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
#[path = "config_tests.rs"]
mod tests;
