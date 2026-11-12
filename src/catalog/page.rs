//! The Store's page in the window: what to show, the listings found, the one picked beside
//! them, and what an install will do before it is done. Its one script runs by its SHA-384, so
//! a page that was altered never runs one; the Store tells it everything as data (`S`).

use serde_json::Value;

/// The page's script, and its SHA-384: the page's policy runs it by that hash.
const SCRIPT: &str = concat!(
    include_str!("../page/kit.js"),
    include_str!("../page/across.js"),
    include_str!("page.js")
);
const SCRIPT_SHA384: &str = "B3Jc7ORcRJZ3XCAtvsyzgCXSrGOjiAMOAf6iKRg3WPWAydAbt55GTAQvaVYHEFNu";

/// The page, titled `title`.
pub(super) fn page(title: &str) -> String {
    let title = crate::page::escaped(title);
    // The script first: the words put in after it are never read as a place for it.
    include_str!("page.html")
        .replacen("{{script}}", SCRIPT, 1)
        .replacen("{{hash}}", SCRIPT_SHA384, 1)
        .replacen("{{title}}", &title, 1)
}

/// The frame that tells the page `what` (`begun`, `found`, `shown`, `row`, `sources`, `said`), with
/// `data`.
pub(super) fn frame(what: &str, data: &Value) -> String {
    format!("S({},{})", Value::from(what), crate::page::json(data))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
