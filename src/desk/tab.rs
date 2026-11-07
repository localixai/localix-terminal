//! Tab keys and the fields each kind accepts at the tool boundary.

use super::args::{TERM_FIELDS, WEB_FIELDS};
use serde_json::Value;

/// What a tab is, from its key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Web,
    Term,
    Board,
}

impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Web => "browser",
            Self::Term => "terminal",
            Self::Board => "board",
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Web => &WEB_FIELDS,
            Self::Term => &TERM_FIELDS,
            Self::Board => &[],
        }
    }

    /// Every field `interact` reads on a tab of this kind, past those of any tab ([`ANY_TAB`]).
    fn takes(self) -> Vec<String> {
        let own = self.fields().iter().map(|field| (*field).to_string());
        match self {
            Self::Web => own.chain(DIALOG.map(String::from)).collect(),
            Self::Term => own.collect(),
            Self::Board => super::args::board_field_names().to_vec(),
        }
    }

    /// How what was meant is done on this kind, after a field it does not take.
    fn instead(self) -> &'static str {
        match self {
            Self::Web => {
                "A control is acted on by its ref from the outline (`[e12]`): `click` it, `fill` \
                 fields (`{\"e4\": \"text\"}`), `select` an option; keys are `keys`, anything \
                 else a `script`."
            }
            Self::Term => "Text is `input`, keys are `keys` ([\"Enter\"], [\"Ctrl-C\"]).",
            Self::Board => "A board is acted on with an `op`: add, take, review, block, release…",
        }
    }
}

/// The fields `interact` reads on any tab: which, the ask of the person, and who acts (a
/// subagent's name, as the caller gives it).
const ANY_TAB: [&str; 4] = ["action", "tab", "ask", "who"];
/// How a page's dialogs are answered.
const DIALOG: [&str; 2] = ["dialog", "dialog_text"];

/// A field `interact` would not read on `kind` is refused, never passed over: a page's
/// `"op": "click"` did nothing, and read as a page that would not answer a click. An `op` that
/// names a field the call carries (`"op": "script"` beside its `script`) says what the call
/// does already, and is let be.
pub(super) fn unread(kind: Kind, args: &Value) -> Result<(), String> {
    let takes = kind.takes();
    let Some(given) = args.as_object() else {
        return Ok(());
    };
    let named = |field: &str| {
        field == "op"
            && kind != Kind::Board
            && (given.get("op").and_then(Value::as_str)).is_some_and(|op| {
                op != "op" && given.contains_key(op) && takes.iter().any(|t| t == op)
            })
    };
    let stray: Vec<String> = (given.keys())
        .filter(|field| !ANY_TAB.contains(&field.as_str()) && !takes.contains(field))
        .filter(|field| !named(field))
        .map(|field| format!("`{field}`"))
        .collect();
    if stray.is_empty() {
        return Ok(());
    }
    let listed: Vec<String> = takes.iter().map(|field| format!("`{field}`")).collect();
    Err(format!(
        "a {} tab takes no {}: nothing was done. It takes {}. {}",
        kind.name(),
        stray.join(", "),
        listed.join(", "),
        kind.instead()
    ))
}

/// A tab of the owner's: its key, kind and number.
pub(super) struct Tab {
    pub(super) key: String,
    pub(super) kind: Kind,
    pub(super) id: u32,
}

/// `w3` or `t2` as a tab.
pub(super) fn parse(key: &str) -> Option<Tab> {
    let key = key.trim().to_ascii_lowercase();
    let kind = match key.chars().next()? {
        'w' => Kind::Web,
        't' => Kind::Term,
        'b' => Kind::Board,
        _ => return None,
    };
    let digits = key.get(1..)?;
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let id = digits.parse().ok()?;
    Some(Tab { key, kind, id })
}

#[cfg(test)]
#[path = "tab_tests.rs"]
mod tests;
