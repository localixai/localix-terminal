//! The menus a right click opens, as the system's (menu.rs). A tab's, on the strip: what its
//! kind can do (a page's address, reloaded, duplicated or opened in the person's own browser),
//! its project's folder shown in the Finder, and closing ("other" and "to the right" are the tabs
//! of the same project: one project's menu never closes another's tabs); and a panel's, in it,
//! of the items its page sends (the terminal's Copy and Paste, a board card's moves), its pick
//! handed back to the page.

use super::On;
use crate::browser::choice::{self, Entry};
use crate::browser::platform;
use crate::browser::strip::Key;
use crate::browser::{Asked, Heard};

/// What the menu offers, and for which tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Reload,
    Duplicate,
    Outside,
    CopyAddress,
    NewTerminal,
    Reveal,
    Split,
    Unsplit,
    Close,
    CloseOthers,
    CloseRight,
}

/// The menu of `groups`, a line between each; `shown`, the tab is the one shown.
fn entries(groups: &[Vec<Item>], shown: bool) -> Vec<Entry> {
    let mut entries = vec![];
    for (at, items) in groups.iter().enumerate() {
        if at > 0 {
            entries.push(Entry::Line { line: true });
        }
        entries.extend(items.iter().map(|item| {
            Entry::Item {
                label: item.title().to_string(),
                marked: false,
                off: false,
                paste: false,
                icon: None,
                // The person's own keys for it, at its right as the menu bar shows them. Close's
                // close the tab shown: beside another tab, they would mislead.
                key: (item.shortcut())
                    .filter(|_| shown || *item != Item::Close)
                    .and_then(crate::browser::shortcuts::keys_of),
            }
        }));
    }
    entries
}

impl Item {
    /// The shortcut that does the same, if one does.
    const fn shortcut(self) -> Option<&'static str> {
        match self {
            Self::Reload => Some("reload"),
            Self::Close => Some("close-tab"),
            Self::Duplicate
            | Self::Outside
            | Self::CopyAddress
            | Self::NewTerminal
            | Self::Reveal
            | Self::Split
            | Self::Unsplit
            | Self::CloseOthers
            | Self::CloseRight => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Reload => "Reload",
            Self::Duplicate => "Duplicate Tab",
            Self::Outside => "Open in Default Browser",
            Self::CopyAddress => "Copy Address",
            Self::NewTerminal => "New Terminal Here",
            Self::Reveal => platform::REVEAL,
            Self::Split => "Open in Split View",
            Self::Unsplit => "Close Split View",
            Self::Close => "Close Tab",
            Self::CloseOthers => "Close Other Tabs",
            Self::CloseRight => "Close Tabs to the Right",
        }
    }
}

/// What is around a tab: whether its project's folder is known, and whether its project has
/// other tabs, and tabs to its right; and the split: whether it can go beside the tab shown
/// (`Some(false)`), or is one of a split window's halves (`Some(true)`).
#[derive(Debug, Clone, Copy, Default)]
struct Around {
    folder: bool,
    others: bool,
    right: bool,
    split: Option<bool>,
}

/// The menu's groups for a tab of `key`'s kind, at `url` (a page's), with `around` it: none of
/// an item it cannot do, a line between groups.
fn groups(key: Key, url: &str, around: Around) -> Vec<Vec<Item>> {
    let page = matches!(key, Key::Web(_));
    let web = page && crate::browser::guard::outside(url);
    let mut groups = vec![];
    if page {
        groups.push(vec![Item::Reload, Item::Duplicate]);
    }
    if web {
        groups.push(vec![Item::Outside, Item::CopyAddress]);
    }
    let mut project = vec![];
    if !page {
        project.push(Item::NewTerminal);
    }
    if around.folder {
        project.push(Item::Reveal);
    }
    groups.push(project);
    groups.push(match around.split {
        Some(true) => vec![Item::Unsplit],
        Some(false) => vec![Item::Split],
        None => vec![],
    });
    let mut close = vec![Item::Close];
    if around.others {
        close.push(Item::CloseOthers);
    }
    if around.right {
        close.push(Item::CloseRight);
    }
    groups.push(close);
    groups.retain(|group| !group.is_empty());
    groups
}

impl On<'_> {
    /// The menu of tab `key`, at `at` of the strip, and what the person picks done.
    pub(super) fn tab_menu(&mut self, key: Key, at: (u32, u32)) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let listed = open.listing(self.shared);
        let name = key.name();
        let Some(this) = listed.iter().position(|tab| tab.id == name) else {
            return;
        };
        let group = listed[this].group.clone();
        let same: Vec<(usize, Key)> = (listed.iter().enumerate())
            .filter(|(_, tab)| tab.group == group)
            .filter_map(|(at, tab)| Some((at, Key::parse(&tab.id)?)))
            .collect();
        let others: Vec<Key> = same
            .iter()
            .map(|&(_, other)| other)
            .filter(|&other| other != key)
            .collect();
        let right: Vec<Key> = same
            .iter()
            .filter(|&&(at, _)| at > this)
            .map(|&(_, other)| other)
            .collect();
        let url = listed[this].detail.clone();
        // A terminal's own folder (where its shell is now), else its project's.
        let folder = (crate::lock(&self.shared.folders).get(&key.name()).cloned()).or_else(|| {
            (crate::lock(&self.shared.places).iter())
                .find(|place| place.key == group)
                .map(|place| place.folder.clone())
        });
        // A tab of the shown one's project (or the window's own) can go beside it; one of the
        // halves can make the window whole.
        let shown = open.shown_key();
        let halves = open
            .pair()
            .is_some_and(|(left, right)| key == left || key == right);
        let alike = (listed.iter().find(|tab| tab.shown))
            .is_some_and(|tab| tab.group == group || group.is_empty() || tab.group.is_empty());
        // Any tab can be split: another goes beside the shown one, the shown one beside the next.
        let split = if halves {
            Some(true)
        } else if (Some(key) != shown && shown.is_some() && alike)
            || (Some(key) == shown && !others.is_empty())
        {
            Some(false)
        } else {
            None
        };
        let around = Around {
            folder: folder.is_some(),
            others: !others.is_empty(),
            right: !right.is_empty(),
            split,
        };
        let groups = groups(key, &url, around);
        let entries = entries(&groups, Some(key) == shown);
        let items: Vec<Item> = groups.into_iter().flatten().collect();
        let at = (f64::from(at.0), f64::from(at.1));
        let Some(picked) = open.pick(at, &entries).and_then(|at| items.get(at)) else {
            return;
        };
        match picked {
            Item::Reload => {
                if let (Key::Web(id), Some(web)) = (key, open.web.as_ref())
                    && let Some(page) = web.at(Some(id))
                {
                    let _ = page.reload();
                }
            }
            Item::Duplicate => {
                self.shared
                    .heard(&group, Heard::Asked(Asked::Page(Some(url))));
            }
            Item::Outside => {
                platform::open_outside(&url);
            }
            Item::CopyAddress => platform::copy(&url),
            Item::NewTerminal => {
                let asked = Asked::Terminal(folder.clone());
                self.shared.heard(&group, Heard::Asked(asked));
            }
            Item::Reveal => {
                if let Some(folder) = folder {
                    platform::reveal(&folder);
                }
            }
            Item::Split if Some(key) == shown => self.split_view(),
            Item::Split => self.split_beside(Some(key)),
            Item::Unsplit => self.split_beside(None),
            Item::Close => self.close(key),
            Item::CloseOthers => self.close_all(others, true),
            Item::CloseRight => self.close_all(right, true),
        }
    }

    /// Panel `id`'s menu of `items`, at `at` of it, and the pick handed to its page: the page's
    /// `chosen(place, pasted)` runs, given the pasteboard's text when the item asks for it, and
    /// the text it answers, if any, is copied. So the page says what its menu does, and the window
    /// knows no panel's items.
    pub(super) fn panel_menu(&mut self, id: u32, at: (u32, u32), items: &[Entry]) {
        let Some(open) = self.deck.as_ref().and_then(|deck| deck.term(id)) else {
            return;
        };
        let at = (f64::from(at.0), f64::from(at.1));
        let Some(place) = platform::show(&open.webview, at, items) else {
            return;
        };
        let pasted = match choice::flat(items).get(place) {
            Some(Entry::Item { paste: true, .. }) => platform::pasted(),
            _ => None,
        };
        let script = format!(
            "window.chosen ? window.chosen({place}, {}) : null",
            serde_json::json!(pasted)
        );
        let _ = open
            .webview
            .evaluate_script_with_callback(&script, |answer| {
                if let Ok(text) = serde_json::from_str::<String>(&answer)
                    && !text.is_empty()
                {
                    platform::copy(&text);
                }
            });
    }
}

#[cfg(test)]
#[path = "host_tab_menu_tests.rs"]
mod tests;
