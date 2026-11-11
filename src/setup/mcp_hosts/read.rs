//! An agent's configuration read for the server's command, as each agent keeps it: JSON (with
//! the comments some of them leave in it), TOML, YAML. Only that one value is looked for, so no
//! parser of the whole format is needed, and the file is never written from here.

use serde_json::Value;

use super::NAME;

/// How an agent's configuration names the server's command.
#[derive(Debug, Clone, Copy)]
pub(super) enum Kept {
    /// JSON: the command (a string) at this path.
    Json(&'static [&'static str]),
    /// JSON: the command line (an array, the program first) at this path.
    JsonLine(&'static [&'static str]),
    /// TOML: `command` in the `[mcp_servers.llx-desk]` table.
    Toml,
    /// YAML: `command:` under `llx-desk:`.
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
fn json(text: &str) -> Option<Value> {
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

/// The heading of the server's table in Codex's TOML.
const TOML_TABLE: &str = "[mcp_servers.llx-desk]";

/// `command` in the server's TOML table (from its heading to the next table), a string of
/// either kind.
fn toml_command(text: &str) -> Option<String> {
    let (_, rest) = text.split_once(TOML_TABLE)?;
    let table = rest.split_once("\n[").map_or(rest, |(table, _)| table);
    let line = table
        .lines()
        .find(|line| line.trim_start().starts_with("command"))?;
    let value = line.split_once('=')?.1.trim();
    // A literal string, in single quotes, is its text as written; a basic one escapes as JSON
    // does. Whatever follows the string (a comment) is not its.
    if let Some(literal) = value.strip_prefix('\'') {
        return literal.split_once('\'').map(|(text, _)| text.to_string());
    }
    serde_json::Deserializer::from_str(value)
        .into_iter::<String>()
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
