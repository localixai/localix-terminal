//! Find in what the window shows (Find…, Find Next, Find Previous of the Edit menu): the tab
//! shown keeps its own find row, query and count in its page, and is told only the step to take
//! (`findText`). A terminal finds in its output; a web page has no find yet, and when it has one
//! it is a branch of `finds` and `find_text` here, so the menu, the keys and Settings stay one
//! way in.

use super::{Deck, Held, Shown};
use crate::browser::strip::{Finding, Key};

impl Deck {
    /// The panel shown, when it finds in itself: a terminal's (`Key::kind`), live in its tab.
    fn finder(&self) -> Option<&wry::WebView> {
        let Shown::Term(id) = self.shown else {
            return None;
        };
        let open = self.term(id).filter(|open| open.held == Held::Live)?;
        (Key::Term(id).kind() == "term").then_some(&open.webview)
    }

    /// Whether what is shown can find: what the menu's Find items are valid for.
    pub(in super::super) fn finds(&self) -> bool {
        self.finder().is_some()
    }

    /// One step of Find in the tab shown, its page given the keys first (the find row's field
    /// takes them). Nothing while a menu or a dialog has the window.
    pub(in super::super) fn find_text(&self, step: Finding) {
        if self.covered() {
            return;
        }
        if let Some(view) = self.finder() {
            let _ = view.focus();
            let call = format!(
                "window.findText && findText({})",
                serde_json::json!(step.word())
            );
            let _ = view.evaluate_script(&call);
        }
    }
}
