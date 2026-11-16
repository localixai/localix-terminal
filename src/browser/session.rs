//! A session in the window (docs/windows.md): a terminal's tab and the documents it holds. The
//! side lists a session by its terminal alone; its documents are a row of tabs over their pane,
//! one of them in sight beside the terminal. All of it is read from the layout as last told, so
//! a tab picked is laid out at once and the layout hears of it after.

use std::collections::HashMap;

use super::strip::Key;
use super::{Arranged, Arranging};

/// The tab of `tabs` that holds pane `key`, in sight or put by.
pub(super) fn holding<'a>(tabs: &'a [Arranging], key: &str) -> Option<&'a Arranging> {
    let holds = |tab: &&Arranging| tab.all.iter().any(|pane| pane == key);
    tabs.iter().find(holds)
}

/// The tab of `arranged` that holds pane `key`, in sight or put by.
pub(super) fn tab_of<'a>(arranged: &'a Arranged, key: &str) -> Option<&'a Arranging> {
    holding(&arranged.tabs, key)
}

/// Whether `key` is a document of `tab`.
fn doc(tab: &Arranging, key: &str) -> bool {
    tab.docs.iter().any(|doc| doc == key)
}

/// The panes of `arranged` that are not listed themselves, each with the pane that stands for
/// its tab in the side: a session's terminal in focus, else its first (the other terminals and
/// the documents are its); of documents with no session, the one in sight (`shown`, the tab
/// the window shows, when it is one of them: the layout hears of it after); of a tab zoomed,
/// the pane drawn.
pub(super) fn led(arranged: &Arranged, shown: Option<&str>) -> HashMap<String, String> {
    let mut led = HashMap::new();
    for tab in arranged.tabs.iter().filter(|tab| !tab.docs.is_empty()) {
        let focused =
            (tab.focused.as_ref()).filter(|pane| tab.all.contains(pane) && !doc(tab, pane));
        let session = focused.or_else(|| tab.all.iter().find(|pane| !doc(tab, pane)));
        let seen = shown.filter(|shown| doc(tab, shown));
        let sight = tab.tree.visible().into_iter().find(|pane| doc(tab, pane));
        let Some(lead) = (session.map(String::as_str)).or(seen).or(sight.as_deref()) else {
            continue;
        };
        let others = match session.is_some() {
            true => &tab.all,
            false => &tab.docs,
        };
        for pane in others.iter().filter(|pane| *pane != lead) {
            led.insert(pane.clone(), lead.to_string());
        }
    }
    // A tab zoomed is one row too, by the pane drawn over it.
    for tab in arranged.tabs.iter().filter(|tab| tab.docs.is_empty()) {
        let drawn = super::split::drawn(tab);
        let [lead] = drawn.as_slice() else {
            continue;
        };
        for pane in tab.all.iter().filter(|pane| *pane != lead) {
            led.insert(pane.clone(), lead.clone());
        }
    }
    led
}

/// The panes of `arranged` whose tab is zoomed (one pane drawn over a tab of several), each
/// with how many panes in sight the zoom puts by: the pane drawn, and the one that stands for
/// the tab in the side.
pub(super) fn zoomed(arranged: &Arranged, shown: Option<&str>) -> HashMap<String, usize> {
    let led = led(arranged, shown);
    let mut zoomed = HashMap::new();
    for tab in arranged.tabs.iter().filter(|tab| tab.whole) {
        let Some(hidden) = tab.tree.visible().len().checked_sub(1).filter(|n| *n > 0) else {
            continue;
        };
        let lead = tab.all.iter().find(|pane| !led.contains_key(*pane));
        for pane in lead.into_iter().chain(&super::split::drawn(tab)) {
            zoomed.insert(pane.clone(), hidden);
        }
    }
    zoomed
}

/// Whether tab `key` is a document: a file or a page, which a session holds.
fn document(key: Key) -> bool {
    matches!(key.kind(), "web" | "file")
}

/// The tab that stands in the side for document `key` while the layout `arranged`, its
/// project's, does not hold it yet (a tab just opened: the layout hears of it a moment after
/// the window draws it): the session of tab `shown`, by its terminal, else the document
/// itself, so no row is lit. Either way it is no row of the side, which never lists a document
/// to take it back once its session is known. None for what the layout holds (`led` says who
/// stands for it), for what is no document, and where the layout says nothing of the project:
/// there is no session to wait for.
pub(super) fn stands_for(
    arranged: Option<&Arranged>,
    key: &str,
    shown: Option<&str>,
) -> Option<String> {
    let arranged = arranged?;
    if !Key::parse(key).is_some_and(document) || tab_of(arranged, key).is_some() {
        return None;
    }
    let session = shown
        .and_then(|shown| tab_of(arranged, shown))
        .and_then(|tab| tab.all.iter().find(|pane| !doc(tab, pane)));
    Some(session.map_or_else(|| key.to_string(), Clone::clone))
}

/// Whether `tab` holds terminals and documents alone: a session, of one terminal or several.
fn sessions(tab: &Arranging) -> bool {
    let terminal = |pane: &String| Key::parse(pane).is_some_and(|key| key.kind() == "term");
    tab.all.iter().all(|pane| doc(tab, pane) || terminal(pane))
}

/// Whether the layout takes tab `beside` put beside tab `shown`: two panes of one tab unless
/// both are its documents (a stack shows one); a document joining a session; a terminal alone
/// joining a session (docs/windows.md: a tab of terminals has one stack); and any two tabs
/// that hold no documents. A session takes no other pane.
pub(super) fn takes(tabs: &[Arranging], shown: Key, beside: Key) -> bool {
    let (shown_name, beside_name) = (shown.name(), beside.name());
    let (mine, other) = (holding(tabs, &shown_name), holding(tabs, &beside_name));
    let (Some(mine), Some(other)) = (mine, other) else {
        return true;
    };
    if std::ptr::eq(mine, other) {
        return !(doc(mine, &shown_name) && doc(mine, &beside_name));
    }
    if beside.kind() == "term" && other.all.len() <= 1 && sessions(mine) {
        return true;
    }
    let terminal = |key: Key, tab: &Arranging| key.kind() == "term" && sessions(tab);
    if (document(beside) && terminal(shown, mine)) || (document(shown) && terminal(beside, other)) {
        return true;
    }
    mine.docs.is_empty() && other.docs.is_empty()
}

/// The documents of each tab of the projects `arranged`, in their order.
pub(super) fn stacks<'a>(arranged: impl IntoIterator<Item = &'a Arranged>) -> Vec<Vec<Key>> {
    (arranged.into_iter())
        .flat_map(|arranged| &arranged.tabs)
        .map(|tab| tab.docs.iter().filter_map(|doc| Key::parse(doc)).collect())
        .filter(|docs: &Vec<Key>| !docs.is_empty())
        .collect()
}

/// The document the window has in sight, and the stack it is of: the tab shown, else one of
/// the panes in `sight` of a split window. The documents' row lies over its pane.
pub(super) fn row<'a>(
    stacks: &'a [Vec<Key>],
    shown: Option<Key>,
    sight: &[Key],
) -> Option<(Key, &'a Vec<Key>)> {
    (shown.into_iter())
        .chain(sight.iter().copied())
        .find_map(|key| Some((key, stacks.iter().find(|stack| stack.contains(&key))?)))
}

/// The document shown once document `key` closes: the one after it in its stack, else the one
/// before.
pub(super) fn next(stacks: &[Vec<Key>], key: Key) -> Option<Key> {
    let stack = stacks.iter().find(|stack| stack.contains(&key))?;
    super::split::partner(stack, key)
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
