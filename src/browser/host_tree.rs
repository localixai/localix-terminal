//! What the person does in the side's tree of files: the files shown or put away, a folder
//! opened or shut, a file opened, and an entry's menu (the system's, menu.rs). A path the page
//! says is acted on only when the tree lists it.

use std::path::Path;

use super::On;
use crate::browser::choice::Entry;
use crate::browser::host::Cmd;
use crate::browser::tree::Said;
use crate::browser::{Asked, Heard, TreeDid};
use crate::browser::{guard, platform};

/// What an entry's menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    OpenTab,
    OpenPage,
    OpenOutside,
    NewTerminal,
    Reveal,
    CopyPath,
    CopyRelative,
}

impl Item {
    fn title(self) -> &'static str {
        match self {
            Self::OpenTab => "Open in New Tab",
            Self::OpenPage => "Open as Page",
            Self::OpenOutside => "Open in Default App",
            Self::NewTerminal => "New Terminal Here",
            Self::Reveal => platform::REVEAL,
            Self::CopyPath => "Copy Path",
            Self::CopyRelative => "Copy Relative Path",
        }
    }
}

/// The menu's groups for the entry at `path` (`rel` under the tree's folder; none: the folder
/// itself), a folder or a file.
fn groups(path: &Path, rel: &str, dir: bool) -> Vec<Vec<Item>> {
    // A file opens in a tab whatever it is (the app says how: an editor, a page); markup has
    // the page it renders as a second way.
    let open = match (dir, guard::renders(path)) {
        (true, _) => vec![Item::NewTerminal],
        (false, true) => vec![Item::OpenTab, Item::OpenPage, Item::OpenOutside],
        (false, false) => vec![Item::OpenTab, Item::OpenOutside],
    };
    let mut place = vec![Item::Reveal, Item::CopyPath];
    if !rel.is_empty() {
        place.push(Item::CopyRelative);
    }
    vec![open, place]
}

fn entries(groups: &[Vec<Item>]) -> Vec<Entry> {
    let mut entries = vec![];
    for (at, items) in groups.iter().enumerate() {
        if at > 0 {
            entries.push(Entry::Line { line: true });
        }
        entries.extend(items.iter().map(|item| Entry::Item {
            label: item.title().to_string(),
            marked: false,
            off: false,
            paste: false,
            icon: None,
            key: None,
        }));
    }
    entries
}

impl On<'_> {
    /// `said` by the side's page done, when it is of its tree of files; whether it was.
    pub(super) fn tree_said(&mut self, said: &str) -> bool {
        let did = crate::browser::tree::said(said);
        if let Some(did) = &did {
            self.tree(did);
        }
        did.is_some()
    }

    /// This window is about to go: the tree it shows is in its sight no more.
    pub(super) fn tree_left(&self) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let unseen = crate::lock(&self.shared.trees).sight(open.tree_window(), None);
        for (project, seen) in unseen {
            (self.shared).heard(&project, Heard::Tree(TreeDid::Seen(seen)));
        }
    }

    /// What the person did in the tree of the project this window shows.
    fn tree(&mut self, said: &Said<'_>) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let project = open.shown_project(self.shared);
        let listed = |path: &str| crate::lock(&self.shared.trees).listed(&project, path);
        match *said {
            Said::Files => {
                {
                    let mut kept = crate::lock(&self.shared.kept);
                    kept.files = Some(!kept.files.unwrap_or(false));
                }
                // Every window's side follows, this one first.
                open.tell_tree(self.shared, None);
                let _ = self.proxy.send_event(Cmd::Strip);
            }
            Said::Open(path) => {
                if listed(path).is_some_and(|(_, dir)| dir) {
                    (self.shared).heard(&project, Heard::Tree(TreeDid::Opened(path)));
                }
            }
            Said::Shut(path) => self
                .shared
                .heard(&project, Heard::Tree(TreeDid::Shut(path))),
            Said::Act(path) => {
                // The app says how a file is shown: an editor, a page, the person's own app.
                if let Some((file, false)) = listed(path) {
                    (self.shared).heard(&project, Heard::Asked(Asked::File(file)));
                }
            }
            Said::Menu(x, y, path) => {
                let Some((at, dir)) = listed(path) else {
                    return;
                };
                let groups = groups(&at, path, dir);
                let items: Vec<Item> = groups.iter().flatten().copied().collect();
                let point = (f64::from(x), f64::from(y));
                let picked = open.pick_in_side(point, &entries(&groups));
                match picked.and_then(|place| items.get(place)) {
                    Some(Item::OpenTab) => {
                        (self.shared).heard(&project, Heard::Asked(Asked::File(at)));
                    }
                    Some(Item::OpenPage) => {
                        if let Ok(address) = url::Url::from_file_path(&at) {
                            let page = Asked::Page(Some(address.into()));
                            (self.shared).heard(&project, Heard::Asked(page));
                        }
                    }
                    Some(Item::OpenOutside) => {
                        if let Ok(address) = url::Url::from_file_path(&at) {
                            platform::open_outside(address.as_str());
                        }
                    }
                    Some(Item::NewTerminal) => {
                        (self.shared).heard(&project, Heard::Asked(Asked::Terminal(Some(at))));
                    }
                    Some(Item::Reveal) => {
                        platform::reveal(&at);
                    }
                    Some(Item::CopyPath) => platform::copy(&at.to_string_lossy()),
                    Some(Item::CopyRelative) => platform::copy(path),
                    None => {}
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "host_tree_tests.rs"]
mod tests;
