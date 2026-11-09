//! The board's page in the window: six columns the person drags cards across, a card opened
//! beside them to edit, accept, send back or comment on. Its one script runs by its SHA-384, so
//! a page that was altered never runs one; the desk paints the board into it whole (`B`), and
//! what it refused (`E`).

use serde_json::json;

use super::Board;

/// The page's script, and its SHA-384: the page's policy runs it by that hash.
const SCRIPT: &str = include_str!("page.js");
const SCRIPT_SHA384: &str = "8p2i3HdshJlb1/1zNdAVBBTtT0NZ7PNHeXKkj+ca0lAVCU5qvix/4IXZhIKVqSAJ";

/// The page, titled `title`.
pub fn page(title: &str) -> String {
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    // The script first: the words put in after it are never read as a place for it.
    include_str!("page.html")
        .replacen("{{script}}", SCRIPT, 1)
        .replacen("{{hash}}", SCRIPT_SHA384, 1)
        .replacen("{{title}}", &title, 1)
}

/// The frame that shows `board` whole: its tab `key`, the project it is `project`'s, and now.
pub fn frame(board: &Board, key: &str, project: &str, now: u64) -> String {
    let state = json!({
        "key": key,
        "project": project,
        "limit": board.limit,
        "now": now,
        "cards": board.cards,
    });
    format!("B({state})")
}

/// The frame that says the desk refused what the person did, and why.
pub fn refused(why: &str) -> String {
    format!("E({})", json!(why))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
