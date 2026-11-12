//! A project's tree of files, as the side shows it. The window holds what the app last told of
//! each tree in sight (`Browser::tree`), so a window opened later, or a page loaded again, is
//! drawn at once; the side's page is told only what is new. A tree is in sight while a window's
//! side shows it, and the app is told as the first window shows it and the last one stops.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::file_icons;
use super::told::{Branch, Tree};

/// What the windows hold of the trees.
#[derive(Default)]
pub(super) struct Trees {
    /// Each tree in sight, by its project's key.
    held: HashMap<String, Held>,
    /// The project whose tree each window shows, by the window.
    sights: HashMap<u64, String>,
    /// What each window's side was last given: the pictures, and a tree whole (`Told`).
    told: HashMap<u64, Told>,
}

/// One tree as last told: its folder, and each open folder's entries by its path.
struct Held {
    root: PathBuf,
    dirs: BTreeMap<String, Branch>,
}

/// What a window's side was given, each for the page load it was given to.
#[derive(Default, Clone, PartialEq, Eq)]
pub(super) struct Told {
    pub(super) pictures: Option<u32>,
    pub(super) tree: Option<(String, u32)>,
}

impl Trees {
    /// `news` of project `key`'s tree taken in; passed over when no window shows it any more.
    pub(super) fn take(&mut self, key: &str, news: Tree) {
        if !self.sights.values().any(|shown| shown == key) {
            return;
        }
        let held = self.held.entry(key.to_string()).or_insert_with(|| Held {
            root: news.root.clone(),
            dirs: BTreeMap::new(),
        });
        if held.root != news.root {
            held.root.clone_from(&news.root);
            held.dirs.clear();
        }
        held.dirs.retain(|path, _| news.open.contains(path));
        for branch in news.dirs {
            if news.open.contains(&branch.path) {
                held.dirs.insert(branch.path.clone(), branch);
            }
        }
    }

    /// Window `window` shows `sight`'s tree from now on (none: no tree). Answers each project
    /// whose tree came into sight (`true`) or went out of every window's (`false`): what is
    /// held of one out of sight is let go of.
    pub(super) fn sight(&mut self, window: u64, sight: Option<&str>) -> Vec<(String, bool)> {
        let was = match sight {
            Some(key) => self.sights.insert(window, key.to_string()),
            None => self.sights.remove(&window),
        };
        if was.as_deref() == sight {
            return vec![];
        }
        let mut changed = vec![];
        if let Some(was) = was
            && !self.sights.values().any(|shown| *shown == was)
        {
            self.held.remove(&was);
            changed.push((was, false));
        }
        if let Some(key) = sight
            && self.sights.values().filter(|shown| *shown == key).count() == 1
        {
            changed.push((key.to_string(), true));
        }
        if sight.is_none() {
            self.told.remove(&window);
        }
        changed
    }

    /// What window `window`'s side was given, to read and to note.
    pub(super) fn told(&mut self, window: u64) -> &mut Told {
        self.told.entry(window).or_default()
    }

    /// The call that draws project `key`'s tree whole; none while the app has told none.
    pub(super) fn whole(&self, key: &str) -> Option<String> {
        let held = self.held.get(key)?;
        let open: Vec<&String> = held.dirs.keys().collect();
        let dirs: Vec<Value> = held.dirs.values().map(drawn).collect();
        Some(call(key, &held.root, true, &json!(open), &dirs))
    }

    /// The file or folder at `path` of project `key`'s tree (`""`: its folder), and whether it
    /// is a folder: only one the tree lists, so nothing the page says leads out of it.
    pub(super) fn listed(&self, key: &str, path: &str) -> Option<(PathBuf, bool)> {
        let held = self.held.get(key)?;
        if path.is_empty() {
            return Some((held.root.clone(), true));
        }
        let (above, name) = path.rsplit_once('/').unwrap_or(("", path));
        let leaf = (held.dirs.get(above)?.leaves.iter()).find(|leaf| leaf.name == name)?;
        Some((held.root.join(path), leaf.dir))
    }
}

/// The call that draws `news` of project `key`'s tree over what the page holds of it.
pub(super) fn news(key: &str, news: &Tree) -> String {
    let dirs: Vec<Value> = news.dirs.iter().map(drawn).collect();
    call(key, &news.root, false, &json!(news.open), &dirs)
}

/// The call that takes the tree out of the side.
pub(super) const NONE: &str = "TR(null)";

fn call(key: &str, root: &Path, whole: bool, open: &Value, dirs: &[Value]) -> String {
    let name = root.file_name().map(|name| name.to_string_lossy());
    let tree = json!({
        "k": key,
        "root": root.to_string_lossy(),
        "name": name,
        "whole": whole,
        "open": open,
        "dirs": dirs,
    });
    format!("TR({tree})")
}

/// A folder's entries as the page draws them: its folders' names, its files each with its
/// picture's name, and how many more it holds.
fn drawn(branch: &Branch) -> Value {
    let (dirs, files): (Vec<_>, Vec<_>) = branch.leaves.iter().partition(|leaf| leaf.dir);
    let dirs: Vec<&str> = dirs.iter().map(|leaf| leaf.name.as_str()).collect();
    let files: Vec<[&str; 2]> = (files.iter())
        .map(|leaf| [leaf.name.as_str(), file_icons::of(&leaf.name)])
        .collect();
    json!({"p": branch.path, "d": dirs, "f": files, "m": branch.more})
}

/// What the person did in the side's tree, as its page says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Said<'a> {
    /// The tree is shown, or put away.
    Files,
    /// A folder opened or shut, by its path.
    Open(&'a str),
    Shut(&'a str),
    /// A file opened (a double click, Return).
    Act(&'a str),
    /// The menu of an entry (`""`: of the tree itself), at this point of the side.
    Menu(u32, u32, &'a str),
}

/// A message of the side's tree: `tree files`, `tree open src/ui`, `tree shut src`, `tree act
/// src/main.rs`, `tree menu 40 212 src/main.rs`. Anything else is not the tree's.
pub(super) fn said(body: &str) -> Option<Said<'_>> {
    let rest = body.strip_prefix("tree ")?;
    let (verb, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    match verb {
        "files" if rest.is_empty() => Some(Said::Files),
        "open" if !rest.is_empty() => Some(Said::Open(rest)),
        "shut" if !rest.is_empty() => Some(Said::Shut(rest)),
        "act" if !rest.is_empty() => Some(Said::Act(rest)),
        "menu" => {
            let mut parts = rest.splitn(3, ' ');
            let (x, y) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            Some(Said::Menu(x, y, parts.next().unwrap_or("")))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
