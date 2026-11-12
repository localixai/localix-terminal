//! What the `window` tool says of itself (its description and JSON Schema, printed by
//! `llx-desk window` with no argument) and the small readers of its arguments.

mod guide;
mod kinds;

pub use guide::description;

use std::sync::OnceLock;

use serde_json::{Map, Value, json};

/// The tool's arguments as a JSON Schema.
pub fn schema() -> Value {
    let mut p = Map::new();
    let mut put = |name: &str, value: Value| {
        p.insert(name.to_string(), value);
    };
    put(
        "action",
        json!({"type": "string", "enum":
        ["open", "interact", "screenshot", "close", "show", "list"]}),
    );
    put(
        "kind",
        json!({"type": "string", "enum": ["browser", "terminal", "board", "file"], "description": "open: the kind of tab."}),
    );
    put(
        "tabs",
        json!({"type": "array", "items": {"anyOf": [{"type": "object"}, {"type": "string"}]}, "description":
        "open: several tabs at once, each an object with `kind` and its fields; close: several tab ids."}),
    );
    put(
        "tab",
        json!({"type": "string", "description": "A tab's id, as `open` or `list` gave it: `w1`, `t2`."}),
    );
    put(
        "dialog",
        json!({"type": "string", "enum": ["accept", "dismiss"], "description": "browser, open and interact: how the page's confirms and prompts are answered during the call (dismiss when not given)."}),
    );
    put(
        "dialog_text",
        json!({"type": "string", "description": "browser: what a prompt is answered with, when `dialog` is accept."}),
    );
    put(
        "url",
        json!({"type": "string", "description": "browser: an address to open, or an HTML file's path."}),
    );
    put(
        "click",
        json!({"anyOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}], "description": "browser: a control's ref from the outline, `e12`, or several in order."}),
    );
    put(
        "fill",
        json!({"type": "object", "additionalProperties": {"type": "string"}, "description": "browser: text to type into fields, by ref: {\"e4\": \"ada@example.com\"}; `{{secret:NAME}}` types a saved secret."}),
    );
    put(
        "select",
        json!({"type": "object", "additionalProperties": {"type": "string"}, "description": "browser: the option to pick in a select, by ref: {\"e7\": \"Large\"}."}),
    );
    put(
        "keys",
        json!({"type": "array", "items": {"type": "string"}, "description": "Keys to press, in order: a page's [\"ArrowUp\", \"Escape\", \"Ctrl+A\"], a terminal's [\"Enter\", \"Up\", \"Ctrl-C\"]."}),
    );
    put(
        "key_delay_ms",
        json!({"type": "integer", "minimum": 0, "maximum": 2000, "description": "browser: ms to wait between `keys`."}),
    );
    put(
        "script",
        json!({"type": "string", "description": "browser: JavaScript run in the page: an expression, or statements whose last value is returned (await works)."}),
    );
    put(
        "read",
        json!({"type": "boolean", "description": "browser: also the page's whole text."}),
    );
    put(
        "offset",
        json!({"type": "integer", "minimum": 0, "description": "browser, read: where in the page's text to go on from."}),
    );
    put(
        "size",
        json!({"type": "string", "description": "The page's size, its tab's own: a device (`iPhone SE`, `iPhone 15 Pro`, `Pixel 8`, `iPad Mini`, `Laptop`, `Desktop`; `mobile`, `tablet`), `WIDTHxHEIGHT`, or `window` to fill the window again. The window stays as the person left it: the page is framed in it. Layout only: the user agent stays."}),
    );
    put(
        "selector",
        json!({"type": "string", "description": "screenshot: crop it to the first element this CSS selector matches."}),
    );
    put(
        "command",
        json!({"type": "string", "description": "terminal: the program to start."}),
    );
    put(
        "workdir",
        json!({"type": "string", "description": "terminal: where to start it, relative to the caller's directory."}),
    );
    put(
        "input",
        json!({"type": "string", "description": "terminal: text to type; a line break is Enter; `{{secret:NAME}}` types a saved secret. At most 4096 bytes."}),
    );
    put(
        "wait_for",
        json!({"type": "string", "description": "Return as soon as the tab shows this text: a page's text, or a control's name or value as the outline gives it."}),
    );
    put(
        "yield_ms",
        json!({"type": "integer", "minimum": 100, "maximum": MAX_YIELD_MS, "description": "The longest the call waits, in ms (a page waits at most 15000)."}),
    );
    p.extend(file_fields());
    p.extend(board_fields());
    p.insert("ask".into(), ask_field());
    (crate::kind::abouts().into_iter()).for_each(|about| kinds::fields(&mut p, about));
    json!({"type": "object", "required": ["action"], "properties": p})
}

/// `interact`'s ask of the person.
pub(super) fn ask_field() -> Value {
    json!({"type": "string", "description": "interact, any tab: what only the person can do there \
        (sign in, a code, a decision), as one line. It waits in their Event Center until they press \
        Done, which reaches you as their note, or comment; one a tab, a new one replaces it, \
        `\"\"` takes it back. Never for news or progress: the Event Center is for what wants them."})
}

/// The fields of opening a file in an editor's tab.
fn file_fields() -> Map<String, Value> {
    let mut p = Map::new();
    let mut put = |name: &str, value: Value| {
        p.insert(name.to_string(), value);
    };
    put(
        "path",
        json!({"type": "string", "description": "file: a text file of the project (UTF-8, up to 2 MB) to open in an editor, relative to the caller's directory; a file has one tab, shown when opened again."}),
    );
    put(
        "line",
        json!({"type": "integer", "description": "file, open: the line to show the person."}),
    );
    p
}

/// The fields of acting on a board's tab.
pub(super) fn board_fields() -> Map<String, Value> {
    let mut p = Map::new();
    let mut put = |name: &str, value: Value| {
        p.insert(name.to_string(), value);
    };
    put(
        "op",
        json!({"type": "string", "enum": ["look", "show", "card", "add", "take", "review", "block", "unblock", "release", "comment", "edit", "remove"], "description": "board: what to do on it; with none, it is read."}),
    );
    put(
        "card",
        json!({"type": "string", "description": "board: a card's id, `c3`."}),
    );
    put(
        "cards",
        json!({"type": "array", "items": {"type": "object"}, "description": "board, add: several cards, each with its `title` and `note`."}),
    );
    put(
        "title",
        json!({"type": "string", "description": "board, add and edit: the card's title, a short line."}),
    );
    put(
        "note",
        json!({"type": "string", "description": "board: add and edit, what the card is about and how to tell it is done; review, what was done; release, where it stands; comment, what to say."}),
    );
    put(
        "to",
        json!({"type": "string", "enum": ["todo", "backlog"], "description": "board, add: Todo (the default), or Backlog for an idea the person may take up later."}),
    );
    put(
        "reason",
        json!({"type": "string", "description": "board, block: what the card waits for."}),
    );
    put(
        "who",
        json!({"type": "string", "description": "board: the worker acting, a subagent's short name; none for the main agent."}),
    );
    p
}

/// The names of the fields of acting on a board's tab.
pub(super) fn board_field_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| board_fields().into_iter().map(|(name, _)| name).collect())
}

/// A string field that says something.
pub(super) fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// A list of strings.
pub(super) fn strings(args: &Value, key: &str) -> Vec<String> {
    (args
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten())
    .filter_map(Value::as_str)
    .map(String::from)
    .collect()
}

pub(super) fn number(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(Value::as_u64)
}

/// The longest `yield_ms` a call may ask a wait for, as the schema says.
const MAX_YIELD_MS: u64 = 30_000;

/// How long a terminal call waits: its `yield_ms` up to [`MAX_YIELD_MS`], else `default`.
pub(super) fn waits(args: &Value, default: std::time::Duration) -> std::time::Duration {
    number(args, "yield_ms").map_or(default, |ms| {
        std::time::Duration::from_millis(ms.min(MAX_YIELD_MS))
    })
}

/// `args` with only `fields`.
pub(super) fn picked(args: &Value, fields: &[&str]) -> Value {
    let kept = fields
        .iter()
        .filter_map(|f| Some(((*f).to_string(), args.get(*f)?.clone())));
    Value::Object(kept.collect())
}

/// The fields each kind reads in `interact`, past those of any tab.
pub(super) const WEB_FIELDS: [&str; 12] = [
    "url",
    "click",
    "fill",
    "select",
    "keys",
    "key_delay_ms",
    "script",
    "read",
    "offset",
    "size",
    "wait_for",
    "yield_ms",
];
pub(super) const TERM_FIELDS: [&str; 4] = ["input", "keys", "wait_for", "yield_ms"];

/// The schema, built once (the kinds are listed before the first call): every call is checked
/// against it.
fn built() -> std::sync::Arc<Value> {
    // A test's thread has its own kinds, so its own schema.
    if cfg!(test) {
        return std::sync::Arc::new(schema());
    }
    static SCHEMA: OnceLock<std::sync::Arc<Value>> = OnceLock::new();
    std::sync::Arc::clone(SCHEMA.get_or_init(|| std::sync::Arc::new(schema())))
}

/// Whether the schema names a field `name`.
pub(super) fn is_field(name: &str) -> bool {
    built()["properties"].get(name).is_some()
}

/// Each argument given is one the schema names, of the type it says, or a message naming the
/// field: an unknown field or a wrong type would otherwise be silently ignored (`"read": "h1"`
/// reading nothing).
pub(super) fn check(args: &Value) -> Result<(), String> {
    let schema = built();
    let (Some(given), Some(fields)) = (args.as_object(), schema["properties"].as_object()) else {
        return Ok(());
    };
    for (name, value) in given {
        let Some(field) = fields.get(name) else {
            // `interact` names what its tab takes instead, once the tab is known.
            if value.is_null() || text(args, "action") == Some("interact") {
                continue;
            }
            let mut known: Vec<&str> = fields.keys().map(String::as_str).collect();
            known.sort_unstable();
            return Err(format!(
                "no field `{name}`: nothing was done. The fields are {}.",
                known.join(", ")
            ));
        };
        let wanted = field["type"].as_str().unwrap_or_default();
        let item = field["items"]["type"].as_str();
        let fits = |v: &Value, t: &str| match t {
            "string" => v.is_string(),
            "integer" => v.is_u64() || v.is_i64(),
            "boolean" => v.is_boolean(),
            "array" => v.is_array(),
            "object" => v.is_object(),
            _ => true,
        };
        let ok = value.is_null()
            || (fits(value, wanted)
                && item.is_none_or(|t| {
                    value
                        .as_array()
                        .is_none_or(|a| a.iter().all(|v| fits(v, t)))
                }));
        if !ok {
            let takes = match item {
                Some(t) => format!("an array of {t}s"),
                None => format!("a {wanted}"),
            };
            let what = field["description"].as_str().unwrap_or_default();
            return Err(format!("`{name}` takes {takes}, not {value}. {what}")
                .trim()
                .to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
