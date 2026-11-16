//! What is open: an owner's tabs, a line each, with what the window says of each and where it
//! lies (`list`), and the window and every owner's tabs (`llxterm status`).

use std::fmt::Write as _;

use super::Desk;
use super::actions::{Kind, parse};

impl Desk {
    /// The owner's tabs, one per line.
    pub(super) fn list(&self, owner: &str) -> String {
        self.prune_pages();
        let keys = crate::lock(&self.registry).tabs_of(owner);
        if keys.is_empty() {
            return "No tabs open.".into();
        }
        let window = self
            .browser
            .as_ref()
            .and_then(crate::browser::Browser::listing)
            .unwrap_or_default();
        let mut out = String::new();
        for key in keys {
            let Some(tab) = parse(&key, &self.made.kinds) else {
                continue;
            };
            let known = window.iter().find(|w| w.id == key);
            let detail = match (tab.kind, known) {
                (Kind::Board, _) => self.board_summary(&tab.key),
                (Kind::File, _) => self.file_summary(&tab.key),
                (Kind::Made(about), _) => self.made_summary(about),
                (_, Some(w)) => format!("{} {}", w.title, w.detail).trim().to_string(),
                (Kind::Term, None) => self
                    .session(tab.id)
                    .map(|s| s.command().to_string())
                    .unwrap_or_default(),
                (Kind::Web, None) => String::new(),
            };
            let shown = if known.is_some_and(|w| w.shown) {
                " [shown]"
            } else {
                ""
            };
            let placed = (self.placed_words(&key)).map_or_else(String::new, |at| format!(" {at}"));
            let _ = writeln!(out, "{key} ({}){shown}{placed}: {detail}", tab.kind.name());
        }
        out.trim_end().to_string()
    }

    /// The window and every owner's tabs, for `llxterm status`.
    pub fn status(&self) -> String {
        let window = match (&self.page, &self.browser) {
            (Some(page), _) if page.is_open() || self.panels_open() => "open",
            // Shown with no tab in it: the window's start of a new tab.
            (Some(_), Some(browser)) if browser.shown() => "open, no tab",
            (Some(_), _) => "not open yet",
            (None, _) => "none (no display, or --no-window)",
        };
        let registry = crate::lock(&self.registry);
        let mut out = format!(
            "window: {window}\nterminals: {}",
            crate::lock(&self.sessions).len()
        );
        if let Some(why) = self.feed.store().kept_in_memory() {
            let _ = write!(
                out,
                "\nstorage: memory only, nothing is kept (database: {why})"
            );
        }
        for (owner, tabs) in registry.owners() {
            let _ = write!(out, "\n{owner}: {}", tabs.join(" "));
        }
        out
    }
}
