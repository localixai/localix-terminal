//! An agent's configuration read for the server's command, as each agent keeps it: JSON (with
//! the comments some of them leave in it), TOML, YAML. Only the values asked for are looked
//! for (a TOML table's string, list of strings or flag), so no parser of the whole format is
//! needed, and the file is never written from here.

use serde_json::Value;

use super::NAME;

/// How an agent's configuration names the server's command.
#[derive(Debug, Clone, Copy)]
pub(super) enum Kept {
    /// JSON: the command (a string) at this path.
    Json(&'static [&'static str]),
    /// JSON: the command line (an array, the program first) at this path.
    JsonLine(&'static [&'static str]),
    /// TOML: `command` in the `[mcp_servers.llxterm]` table.
    Toml,
    /// YAML: `command:` under `llxterm:`.
    Yaml,
}

impl Kept {
    /// The command `text` (a configuration) runs for the server, if it has one.
    pub(super) fn entry(self, text: &str) -> Option<String> {
        match self {
            Self::Json(at) => at_path(&json(text)?, at)?.as_str().map(String::from),
            Self::JsonLine(at) => at_path(&json(text)?, at)?
                .get(0)?
                .as_str()
                .map(String::from),
            Self::Toml => toml_command(text),
            Self::Yaml => yaml_command(text),
        }
    }
}

/// Whether `text`, a JSON configuration, has a value at `path`.
pub(super) fn has(text: &str, path: &[&str]) -> bool {
    json(text).is_some_and(|config| at_path(&config, path).is_some_and(|value| !value.is_null()))
}

/// `text` as JSON, with or without comments.
pub(in crate::setup) fn json(text: &str) -> Option<Value> {
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

/// The value at `path` in `config`.
fn at_path<'a>(config: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(config, |value, key| value.get(*key))
}

/// The server's table in Codex's TOML.
const TOML_TABLE: &str = "mcp_servers.llxterm";

/// `command` in the server's TOML table.
fn toml_command(text: &str) -> Option<String> {
    toml_value(text, TOML_TABLE, "command")?
        .as_str()
        .map(String::from)
}

/// Key `key` of table `table` in a TOML text (from the table's heading, `[table]`, to the next
/// heading), when it is a string of either kind, a list of basic strings or a flag on one line,
/// as the agents' own commands write them. Whatever follows the value (a comment) is not its.
pub(in crate::setup) fn toml_value(text: &str, table: &str, key: &str) -> Option<Value> {
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
#[path = "read_tests.rs"]
mod tests;
