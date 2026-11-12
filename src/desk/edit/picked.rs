//! A file the person picked in the project's tree, opened as its kind asks: text in an
//! editor's tab; a picture written as text, and what no editor takes, as a page, which shows
//! it or says in one line that nothing does. Nothing is handed to another app unasked.

use std::path::Path;

use crate::desk::{Call, Desk};

impl Desk {
    /// The file at `path` opened for the person, who picked it in the project's tree: text in
    /// an editor's tab; a picture written as text, and what no editor takes (a picture, a film,
    /// an archive), as a page, which shows it or says in a line that nothing does. It joins the
    /// session they look at (beside.rs), where their call names no pane.
    pub(in crate::desk) fn person_file(&self, call: &Call, path: &Path) -> Result<(), String> {
        let inside = (path.canonicalize()).map_err(|why| format!("{}: {why}", path.display()))?;
        let page = || {
            let page = serde_json::json!({"kind": "browser", "url": inside});
            (self.open(call, &page))
                .map(drop)
                .map_err(|failure| failure.message().to_string())
        };
        if crate::browser::drawn(&inside) {
            return page();
        }
        match self.open_file(call, &inside, None) {
            Ok(_) => Ok(()),
            Err(not) if not.page => page(),
            Err(not) => Err(not.why),
        }
    }

    /// The file at `path` opened for the person as its text, in an editor's tab: the second
    /// way of a picture written as text.
    pub(in crate::desk) fn person_text(&self, call: &Call, path: &Path) -> Result<(), String> {
        let inside = (path.canonicalize()).map_err(|why| format!("{}: {why}", path.display()))?;
        (self.open_file(call, &inside, None))
            .map(drop)
            .map_err(|not| not.why)
    }
}

#[cfg(test)]
#[path = "picked_tests.rs"]
mod tests;
