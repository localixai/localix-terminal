//! What the person does in the side's tree of files: the files shown or put away, a folder
//! opened or shut, a file opened, an entry made, named, moved, thrown away or dragged out over
//! a pane, and an entry's menu (the system's, menu.rs). A path the page says is acted on only
//! when the tree lists it.

use std::path::Path;

use super::On;
use crate::browser::choice::Entry;
use crate::browser::host::Cmd;
use crate::browser::tree::{self, Said};
use crate::browser::{Asked, EntryDid, Heard, TreeDid};
use crate::browser::{guard, platform};

/// What an entry's menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    NewFile,
    NewFolder,
    OpenTab,
    OpenPage,
    OpenText,
    OpenOutside,
    NewTerminal,
    Reveal,
    Rename,
    Trash,
    Give,
    CopyPath,
    CopyRelative,
}

impl Item {
    fn title(self) -> &'static str {
        match self {
            Self::NewFile => "New File…",
            Self::NewFolder => "New Folder…",
            Self::OpenTab => "Open in New Tab",
            Self::OpenPage => "Open as Page",
            Self::OpenText => "Open as Text",
            Self::OpenOutside => "Open in Default App",
            Self::NewTerminal => "New Terminal Here",
            Self::Reveal => platform::REVEAL,
            Self::Rename => "Rename…",
            Self::Trash => "Move to Trash",
            Self::Give => "Insert Path in Terminal",
            Self::CopyPath => "Copy Path",
            Self::CopyRelative => "Copy Relative Path",
        }
    }
}

/// The menu's groups for the entry at `path` (`rel` under the tree's folder; none: the folder
/// itself), a folder or a file: what it opens or holds, its place and name, its path given to
/// the terminal in sight, and its path copied.
fn groups(path: &Path, rel: &str, dir: bool) -> Vec<Vec<Item>> {
    // A file opens in a tab whatever it is (the app says how: an editor, a page). Markup has a
    // second way: the page it renders, or, for a picture a pick shows drawn, its text.
    let second = match (guard::renders(path), guard::drawn(path)) {
        (true, false) => Some(Item::OpenPage),
        (true, true) => Some(Item::OpenText),
        (false, _) => None,
    };
    let mut groups = match dir {
        true => vec![
            vec![Item::NewFile, Item::NewFolder],
            vec![Item::NewTerminal],
        ],
        false => {
            let ways = [Some(Item::OpenTab), second, Some(Item::OpenOutside)];
            vec![ways.into_iter().flatten().collect()]
        }
    };
    // The tree's own folder is the project: neither named anew nor thrown away from here.
    let (mut place, mut copy) = (vec![Item::Reveal], vec![Item::CopyPath]);
    if !rel.is_empty() {
        place.extend([Item::Rename, Item::Trash]);
        copy.push(Item::CopyRelative);
    }
    groups.extend([place, vec![Item::Give], copy]);
    groups
}

/// The menu's rows; a path is given only where a terminal is in sight to take it (`terminal`).
fn entries(groups: &[Vec<Item>], terminal: bool) -> Vec<Entry> {
    let mut entries = vec![];
    for (at, items) in groups.iter().enumerate() {
        if at > 0 {
            entries.push(Entry::Line { line: true });
        }
        entries.extend(items.iter().map(|item| Entry::Item {
            label: item.title().to_string(),
            marked: false,
            off: *item == Item::Give && !terminal,
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
        let did = tree::said(said);
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
            Said::Menu(x, y, path) => self.tree_menu(&project, (x, y), path),
            Said::Make(dir, name, folder) => {
                if listed(dir).is_some_and(|(_, dir)| dir) {
                    self.entry(&project, EntryDid::Made { dir, name, folder });
                }
            }
            Said::Name(path, name) => {
                if listed(path).is_some() {
                    self.entry(&project, EntryDid::Named { path, name });
                }
            }
            Said::Move(path, into) => {
                if listed(path).is_some() && listed(into).is_some_and(|(_, dir)| dir) {
                    self.entry(&project, EntryDid::Moved { path, into });
                }
            }
            Said::Trash(path) => {
                if listed(path).is_some() {
                    self.entry(&project, EntryDid::Trashed(path));
                }
            }
            Said::Drop(x, y, path) => {
                let Some((at, dir)) = listed(path) else {
                    return;
                };
                // Let go over a terminal, its path is typed there; a file let go over any
                // other pane opens, as one picked in the tree does.
                let over = open.pane_under(f64::from(x), f64::from(y) + open.top());
                match over.filter(|pane| pane.kind() == "term") {
                    Some(terminal) => {
                        let terminal = &terminal.name();
                        self.entry(&project, EntryDid::Given { path, terminal });
                    }
                    None if over.is_some() && !dir => {
                        (self.shared).heard(&project, Heard::Asked(Asked::File(at)));
                    }
                    None => {}
                }
            }
        }
    }

    /// What the person did to an entry of `project`'s tree, told to the app.
    fn entry(&self, project: &str, did: EntryDid<'_>) {
        (self.shared).heard(project, Heard::Tree(TreeDid::Entry(did)));
    }

    /// The menu of the entry at `path` of `project`'s tree, at `point` of the side, and what
    /// is picked in it done.
    fn tree_menu(&self, project: &str, (x, y): (u32, u32), path: &str) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let Some((at, dir)) = crate::lock(&self.shared.trees).listed(project, path) else {
            return;
        };
        let terminal = open.session_terminal(self.shared);
        let groups = groups(&at, path, dir);
        let items: Vec<Item> = groups.iter().flatten().copied().collect();
        let point = (f64::from(x), f64::from(y));
        let picked = open.pick_in_side(point, &entries(&groups, terminal.is_some()));
        let Some(item) = picked.and_then(|place| items.get(place)) else {
            return;
        };
        match item {
            // The page takes the name: in the row itself, or in a row of its own.
            Item::NewFile | Item::NewFolder => {
                let folder = *item == Item::NewFolder;
                open.tell_side(&tree::writes(project, path, Some(folder)));
            }
            Item::Rename => open.tell_side(&tree::writes(project, path, None)),
            Item::Trash => self.entry(project, EntryDid::Trashed(path)),
            Item::Give => {
                if let Some(terminal) = &terminal {
                    self.entry(project, EntryDid::Given { path, terminal });
                }
            }
            Item::OpenTab => (self.shared).heard(project, Heard::Asked(Asked::File(at))),
            Item::OpenText => (self.shared).heard(project, Heard::Asked(Asked::Text(at))),
            Item::OpenPage => {
                if let Ok(address) = url::Url::from_file_path(&at) {
                    let page = Asked::Page(Some(address.into()));
                    (self.shared).heard(project, Heard::Asked(page));
                }
            }
            Item::OpenOutside => {
                if let Ok(address) = url::Url::from_file_path(&at) {
                    platform::open_outside(address.as_str());
                }
            }
            Item::NewTerminal => {
                (self.shared).heard(project, Heard::Asked(Asked::Terminal(Some(at))));
            }
            Item::Reveal => {
                platform::reveal(&at);
            }
            Item::CopyPath => platform::copy(&at.to_string_lossy()),
            Item::CopyRelative => platform::copy(path),
        }
    }
}

#[cfg(test)]
#[path = "host_tree_tests.rs"]
mod tests;
