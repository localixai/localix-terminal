//! What a menu offers, for every platform: items, lines between groups, and submenus. A page of
//! a panel sends its own (the board's card menu, the terminal's), the strip's buttons theirs; the
//! system's menu shows them (menu.rs, menu_gtk.rs) and answers the place of the item picked.

use serde::Deserialize;

/// One line of a menu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(super) enum Entry {
    /// A submenu under `label`.
    Sub { label: String, items: Vec<Entry> },
    /// An item: picked, the menu answers its place among the items (lines and submenus' titles
    /// not counted, a submenu's items counted where they are).
    Item {
        label: String,
        /// Shown with a check mark.
        #[serde(default)]
        marked: bool,
        /// Shown, and not picked.
        #[serde(default)]
        off: bool,
        /// Picked, the pasteboard's text goes to the page with it (a terminal's Paste).
        #[serde(default)]
        paste: bool,
        /// A picture beside it: an SVG in one colour, tinted as the item's text (macOS). Never
        /// from a page: the window's own menus alone set it.
        #[serde(skip)]
        icon: Option<&'static str>,
        /// Its keys, shown at its right as the system shows a shortcut (`cmd+t`); only a label:
        /// the keys themselves are the window's, whether or not the menu is open.
        #[serde(default)]
        key: Option<String>,
    },
    /// A line between groups.
    Line { line: bool },
}

/// `labels` as items, the one at `marked` checked.
pub(super) fn items(labels: &[&str], marked: Option<usize>) -> Vec<Entry> {
    (labels.iter().enumerate())
        .map(|(at, label)| Entry::Item {
            label: (*label).to_string(),
            marked: marked == Some(at),
            off: false,
            paste: false,
            icon: None,
            key: None,
        })
        .collect()
}

/// The items of `entries` in order, a submenu's in its place: what a pick's place counts.
pub(super) fn flat(entries: &[Entry]) -> Vec<&Entry> {
    let mut out = vec![];
    for entry in entries {
        match entry {
            Entry::Sub { items, .. } => out.extend(flat(items)),
            Entry::Item { .. } => out.push(entry),
            Entry::Line { .. } => {}
        }
    }
    out
}

#[cfg(test)]
#[path = "choice_tests.rs"]
mod tests;
