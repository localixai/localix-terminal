//! Which project the window shows: its tabs are the ones listed, what the person opens is of it,
//! and its history is the one the menu lists. A tab an agent opens or shows in another project
//! stays out of sight there, and that project is marked in the switcher until the person goes
//! to it: the window never jumps from what the person looks at.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone)]
pub(super) struct Scope {
    /// The project shown, by its key; empty until the window shows a tab.
    shown: String,
    /// Each project's tab shown last, so going back to it shows that one.
    last: HashMap<String, String>,
    /// The projects where something happened out of sight.
    marked: HashSet<String>,
    /// The project of the next tab the app opens (`Browser::opening`), and whether that tab
    /// opens beside one in sight and so does not take the window.
    opening: Option<(String, bool)>,
    /// The terminals and boards that open beside a tab in sight, by their keys.
    aside: HashSet<String>,
}

impl Scope {
    /// The project shown (empty: none yet).
    pub(super) fn shown(&self) -> &str {
        &self.shown
    }

    /// Whether a tab of `project` is in sight: it is of the project shown, or the window shows
    /// none yet.
    pub(super) fn in_sight(&self, project: &str) -> bool {
        self.shown.is_empty() || self.shown == project
    }

    /// Something happened in a tab of `project`: marked, when it is out of sight.
    pub(super) fn touched(&mut self, project: &str) {
        if !self.in_sight(project) {
            self.marked.insert(project.to_string());
        }
    }

    /// Whether `project` is marked.
    pub(super) fn marked(&self, project: &str) -> bool {
        self.marked.contains(project)
    }

    /// Tab `key` of `project` is shown: that project is the one shown, and this its last tab.
    /// Whether it is another project than before.
    pub(super) fn showed(&mut self, project: &str, key: &str) -> bool {
        let moved = self.shown != project;
        project.clone_into(&mut self.shown);
        self.last.insert(project.to_string(), key.to_string());
        self.marked.remove(project);
        moved
    }

    /// The person goes to `project`, whose tabs are `open` (in order): the tab to show, its
    /// last one while it is open, else its first; none when it has none, and then the window
    /// shows no tab.
    pub(super) fn go(&mut self, project: &str, open: &[String]) -> Option<String> {
        project.clone_into(&mut self.shown);
        self.marked.remove(project);
        let last = self.last.get(project).filter(|key| open.contains(key));
        last.or_else(|| open.first()).cloned()
    }

    /// The next tab the app opens is of `project`; `aside`, it opens beside one in sight and
    /// does not take the window.
    pub(super) fn opening(&mut self, project: &str, aside: bool) {
        self.opening = Some((project.to_string(), aside));
    }

    /// The project of the tab opening now and whether it opens aside, once.
    pub(super) fn take_opening(&mut self) -> Option<(String, bool)> {
        self.opening.take()
    }

    /// Whether the tab opening next opens aside: asked before it is taken.
    pub(super) fn opens_aside(&self) -> bool {
        self.opening.as_ref().is_some_and(|(_, aside)| *aside)
    }

    /// Tab `key`, about to open, opens beside one in sight.
    pub(super) fn aside(&mut self, key: &str) {
        self.aside.insert(key.to_string());
    }

    /// Whether tab `key` opens aside, once.
    pub(super) fn take_aside(&mut self, key: &str) -> bool {
        self.aside.remove(key)
    }

    /// What of this scope is its window's (the project it shows, each project's last tab in
    /// it) traded with `other`'s: the window in use is another. The marks and
    /// the project of the tab opening next are the app's, and stay.
    pub(super) fn trade_window(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.shown, &mut other.shown);
        std::mem::swap(&mut self.last, &mut other.last);
    }

    /// A new window's scope: the project this one shows, no tab of it shown yet.
    pub(super) fn fresh(&self) -> Self {
        Self {
            shown: self.shown.clone(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
