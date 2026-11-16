//! What the window calls in the page shown that nobody waits for (call 0, which the probe does
//! not answer): commenting turned on or off, and a step of find; and what a page's find says
//! back of its own accord. What the person typed reaches the page as data (JSON), never as
//! script.

use serde_json::{Value, json};

/// The call that turns commenting on or off in a page, with the run's call `key`; a page whose
/// marks are of an earlier batch than `sent` drops them.
pub(in crate::browser) fn comment(key: &str, on: bool, sent: u32) -> String {
    format!(
        "{}, 'comment', {{ on: {on}, sent: {sent} }}) : false",
        crate::browser::script::head(key, 0, "")
    )
}

/// A step of find in the page of tab `tab` (`type`, `next`, `previous`, `again`, `clear`), of
/// `query`, lit in `accent`. The page says what it found as a `Found` for that tab.
pub(in crate::browser) fn find(
    key: &str,
    tab: u32,
    step: &str,
    query: &str,
    accent: &str,
) -> String {
    let args = json!({ "tab": tab, "step": step, "query": query, "accent": accent });
    format!(
        "{}, 'find', {}) : false",
        crate::browser::script::head(key, 0, ""),
        crate::page::json(&args)
    )
}

/// What a page found: for which tab, of which query, the match shown (-1: none) of how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::browser) struct Found {
    pub(in crate::browser) tab: u32,
    pub(in crate::browser) query: String,
    pub(in crate::browser) at: i64,
    pub(in crate::browser) of: u32,
}

/// What a page's probe said it found (`{tab, query, at, of}`), held to what a find can say: the
/// match shown is one of those counted, or none.
pub(in crate::browser) fn found(said: &Value) -> Option<Found> {
    let tab = u32::try_from(said.get("tab")?.as_u64()?).ok()?;
    let query = said.get("query")?.as_str()?.to_string();
    let of = u32::try_from(said.get("of")?.as_u64()?).ok()?;
    let at = said.get("at")?.as_i64()?;
    (at == -1 || (0..i64::from(of)).contains(&at)).then_some(Found { tab, query, at, of })
}
