//! The board's page in the window: six columns the person drags cards across, a card opened
//! beside them to edit, accept, send back or comment on. Its one script runs by its SHA-384, so
//! a page that was altered never runs one; the desk paints the board into it whole (`B`), and
//! what it refused (`E`).

use serde_json::{Value, json};

use super::Board;

/// The page's script after the pages' kit, and their SHA-384: the page's policy runs it by that hash.
const SCRIPT: &str = concat!(include_str!("../page/kit.js"), include_str!("page.js"));
const SCRIPT_SHA384: &str = "HtWdY0SK4h1KqDc4KhYcoZUm1jvj2lMySy236tX1aM5rouap7GZTrVKIvMM4lE4+";

/// The page, titled `title`.
pub fn page(title: &str) -> String {
    let title = crate::page::escaped(title);
    // The script first: the words put in after it are never read as a place for it.
    include_str!("page.html")
        .replacen("{{script}}", SCRIPT, 1)
        .replacen("{{hash}}", SCRIPT_SHA384, 1)
        .replacen("{{title}}", &title, 1)
}

/// The frame that shows `board` whole: its tab `key`, the project it is `project`'s, now, and
/// the coding agents installed that a card can be run by (`[{"id", "name"}]`).
pub fn frame(board: &Board, key: &str, project: &str, now: u64, agents: &Value) -> String {
    let state = json!({
        "key": key,
        "project": project,
        "limit": board.limit,
        "now": now,
        "cards": board.cards,
        "agents": agents,
    });
    format!("B({})", crate::page::json(&state))
}

/// The frame that says the desk refused what the person did, and why.
pub fn refused(why: &str) -> String {
    format!("E({})", crate::page::json(&json!(why)))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
