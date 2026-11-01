//! The menus a right click opens, as the system's (menu.rs). A tab's, on the strip, as Safari's
//! ("other" and "to the right" are the tabs of the same project: one project's menu never closes
//! another's tabs); and a terminal's, in it, as Terminal's: Copy, Paste, Select All, Clear.

use super::super::Cmd;
use super::On;
use crate::browser::panel::Selection;
use crate::browser::platform;
use crate::browser::strip::Key;

/// What a terminal's menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Term {
    Copy,
    Paste,
    SelectAll,
    Clear,
}

impl Term {
    fn title(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Clear => "Clear",
        }
    }
}

/// What the menu offers, and for which tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Reload,
    Duplicate,
    Close,
    CloseOthers,
    CloseRight,
}

impl Item {
    fn title(self) -> &'static str {
        match self {
            Self::Reload => "Reload",
            Self::Duplicate => "Duplicate Tab",
            Self::Close => "Close Tab",
            Self::CloseOthers => "Close Other Tabs",
            Self::CloseRight => "Close Tabs to the Right",
        }
    }
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
        let group = &listed[this].group;
        let same: Vec<(usize, Key)> = (listed.iter().enumerate())
            .filter(|(_, tab)| &tab.group == group)
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
        let mut items = vec![];
        if let Key::Web(_) = key {
            items.extend([Item::Reload, Item::Duplicate]);
        }
        items.push(Item::Close);
        if !others.is_empty() {
            items.push(Item::CloseOthers);
        }
        if !right.is_empty() {
            items.push(Item::CloseRight);
        }
        let titles: Vec<&str> = items.iter().map(|item| item.title()).collect();
        let at = (f64::from(at.0), f64::from(at.1));
        let Some(picked) = open.choose(at, &titles).and_then(|at| items.get(at)) else {
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
                let _ = self.proxy.send_event(Cmd::NewTab { url, reply: None });
            }
            Item::Close => self.close(key),
            Item::CloseOthers => others.into_iter().for_each(|other| self.close(other)),
            Item::CloseRight => right.into_iter().for_each(|other| self.close(other)),
        }
    }

    /// Terminal `id`'s menu, at `at` of its page, and what the person picks done. Copy and paste
    /// go through the pasteboard: the page's selection read, the pasteboard's text pasted as
    /// typed (emulator.js's `term`, which sends it to the session's keyboard).
    pub(super) fn panel_menu(&mut self, id: u32, at: (u32, u32), selection: Selection) {
        let Some(open) = self.deck.as_ref().and_then(|deck| deck.term(id)) else {
            return;
        };
        let mut items = vec![];
        if selection == Selection::Some {
            items.push(Term::Copy);
        }
        items.extend([Term::Paste, Term::SelectAll, Term::Clear]);
        let titles: Vec<&str> = items.iter().map(|item| item.title()).collect();
        let at = (f64::from(at.0), f64::from(at.1));
        let picked = platform::choose(&open.webview, at, &titles).and_then(|at| items.get(at));
        let view = &open.webview;
        let _ = match picked {
            Some(Term::Copy) => view.evaluate_script_with_callback("term.getSelection()", |text| {
                if let Ok(text) = serde_json::from_str::<String>(&text) {
                    platform::copy(&text);
                }
            }),
            Some(Term::Paste) => match platform::pasted() {
                Some(text) => {
                    view.evaluate_script(&format!("term.paste({})", serde_json::json!(text)))
                }
                None => Ok(()),
            },
            Some(Term::SelectAll) => view.evaluate_script("term.selectAll()"),
            // Ctrl-L, as typed: the program clears its screen (a shell redraws its prompt).
            Some(Term::Clear) => view.evaluate_script(r#"window.ipc.postMessage("k\f")"#),
            None => Ok(()),
        };
    }
}
