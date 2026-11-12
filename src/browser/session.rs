//! A session in the window (docs/windows.md): a terminal's tab and the documents it holds. The
//! side lists a session by its terminal alone; its documents are a row of tabs over their pane,
//! one of them in sight beside the terminal. All of it is read from the layout as last told, so
//! a tab picked is laid out at once and the layout hears of it after.

use std::collections::HashMap;

use super::split::Kept;
use super::strip::Key;
use super::{Arranged, Arranging};

/// The tab of `arranged` that holds pane `key`, in sight or put by.
pub(super) fn tab_of<'a>(arranged: &'a Arranged, key: &str) -> Option<&'a Arranging> {
    let holds = |tab: &&Arranging| tab.all.iter().chain(&tab.panes).any(|pane| pane == key);
    arranged.tabs.iter().find(holds)
}

/// Whether `key` is a document of `tab`.
fn doc(tab: &Arranging, key: &str) -> bool {
    tab.docs.iter().any(|doc| doc == key)
}

/// The documents of `arranged` that are not listed themselves, each with the pane that stands
/// for its tab in the side: a session's terminal; of documents with no session, the one in
/// sight (`shown`, the tab the window shows, when it is one of them: the layout hears of it
/// after).
pub(super) fn led(arranged: &Arranged, shown: Option<&str>) -> HashMap<String, String> {
    let mut led = HashMap::new();
    for tab in arranged.tabs.iter().filter(|tab| !tab.docs.is_empty()) {
        let session = tab.all.iter().find(|pane| !doc(tab, pane));
        let seen = shown.filter(|shown| doc(tab, shown));
        let sight = tab.panes.iter().find(|pane| doc(tab, pane));
        let Some(lead) = (session.map(String::as_str))
            .or(seen)
            .or_else(|| sight.map(String::as_str))
        else {
            continue;
        };
        for doc in tab.docs.iter().filter(|doc| *doc != lead) {
            led.insert(doc.clone(), lead.to_string());
        }
    }
    led
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

/// The split the window lays tab `shown` out in, as the layout keeps its tab: the pane beside
/// it; a document put by takes the place of the one in sight. None for a tab of one pane, one
/// the person keeps whole, or one the layout does not hold.
pub(super) fn split_of(arranged: &Arranged, shown: &str) -> Option<Kept> {
    let tab = tab_of(arranged, shown).filter(|tab| !tab.whole)?;
    pair_of(tab, shown)
}

/// The split tab `shown` comes back to when the person splits a session they kept whole.
pub(super) fn beside_whole(arranged: &Arranged, shown: &str) -> Option<Kept> {
    let tab = tab_of(arranged, shown).filter(|tab| tab.whole)?;
    pair_of(tab, shown)
}

fn pair_of(tab: &Arranging, shown: &str) -> Option<Kept> {
    let [left, right] = tab.panes.as_slice() else {
        return None;
    };
    let at = match tab.panes.iter().position(|pane| pane == shown) {
        Some(at) => at,
        None if doc(tab, shown) => tab.panes.iter().position(|pane| doc(tab, pane))?,
        None => return None,
    };
    Some(Kept {
        shown: shown.to_string(),
        beside: [right, left][at].clone(),
        right: at == 0,
    })
}

/// Whether the layout takes tab `beside` put beside tab `shown`: two panes of one tab unless
/// both are its documents (a stack shows one); a document joining a terminal; and any two
/// tabs that hold no documents. A session takes no other pane.
pub(super) fn takes(arranged: &Arranged, shown: Key, beside: Key) -> bool {
    let (shown_name, beside_name) = (shown.name(), beside.name());
    let (mine, other) = (
        tab_of(arranged, &shown_name),
        tab_of(arranged, &beside_name),
    );
    let (Some(mine), Some(other)) = (mine, other) else {
        return true;
    };
    if std::ptr::eq(mine, other) {
        return !(doc(mine, &shown_name) && doc(mine, &beside_name));
    }
    let terminal = |key: Key, tab: &Arranging| {
        key.kind() == "term" && (!tab.docs.is_empty() || tab.all.len() <= 1)
    };
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

/// The document the window has in sight, and the stack it is of: the tab shown, else the one
/// beside it in a split window. The documents' row lies over its pane.
pub(super) fn row(
    stacks: &[Vec<Key>],
    shown: Option<Key>,
    pair: Option<(Key, Key)>,
) -> Option<(Key, &Vec<Key>)> {
    let halves = pair.map(|(left, right)| [left, right]);
    (shown.into_iter())
        .chain(halves.into_iter().flatten())
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
