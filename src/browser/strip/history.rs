//! The strip's history menu: the project's tabs as its ledger keeps them, newest first, each with
//! what became of it, drawn by the strip's page as data.

use serde_json::json;

use super::Key;

/// What became of a tab the history menu lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::browser) enum State {
    Open,
    /// A terminal the person closed: its program runs on, its screen live out of sight.
    Hidden,
    /// A terminal whose session ended: its last screen is kept.
    Ended,
    /// A browser tab closed: its page opens again in a new one.
    Closed,
}

/// An entry of the history menu: its row in the project's ledger, and its tab's key while open.
pub(in crate::browser) struct Row {
    pub at: i64,
    pub key: Option<Key>,
    /// What it is, as the strip names it: `web`, `term` or `board`.
    pub kind: &'static str,
    pub title: String,
    /// A browser tab's page.
    pub detail: String,
    pub state: State,
    pub group: String,
}

/// The call that opens the history menu on `rows` (newest first), or closes it.
pub(in crate::browser) fn menu(rows: Option<&[Row]>) -> String {
    let shown = rows.map(|rows| {
        let rows: Vec<_> = rows
            .iter()
            .map(|row| {
                let state = match row.state {
                    State::Open => "open",
                    State::Hidden => "hidden",
                    State::Ended => "ended",
                    State::Closed => "closed",
                };
                json!({"at": row.at, "key": row.key.map(Key::name), "kind": row.kind, "title": row.title,
                    "detail": row.detail, "state": state, "group": row.group})
            })
            .collect();
        rows
    });
    format!("M({})", json!(shown))
}
