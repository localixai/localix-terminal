//! Which project the window shows: its tabs are the ones listed, what the person opens is of it,
//! and its history is the one the menu lists. A tab an agent opens or shows in another project
//! stays out of sight there, and that project is marked in the switcher until the person goes
//! to it: the window never jumps from what the person looks at.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// How long the window waits for a project's tabs to come back before it follows the tab it
/// shows again: a restore that failed never leaves it waiting.
const AWAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Default, Clone)]
pub(super) struct Scope {
    /// The project shown, by its key; empty until the window shows a tab.
    shown: String,
    /// Each project's tab shown last, so going back to it shows that one.
    last: HashMap<String, String>,
    /// The projects where something happened out of sight.
    marked: HashSet<String>,
    /// The project of the next tab the app opens (`Browser::opening`).
    opening: Option<String>,
    /// The person went to a project with no tab open: the app opens what it had, and till a
    /// tab of it shows (or [`AWAIT`] passed), the window does not follow the tab it shows
    /// elsewhere. When the person went.
    awaiting: Option<Instant>,
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
        self.awaiting = None;
        project.clone_into(&mut self.shown);
        self.last.insert(project.to_string(), key.to_string());
        self.marked.remove(project);
        moved
    }

    /// The person goes to `project`, whose tabs are `open` (in order): the tab to show, its
    /// last one while it is open, else its first; none when it has none.
    pub(super) fn go(&mut self, project: &str, open: &[String]) -> Option<String> {
        project.clone_into(&mut self.shown);
        self.marked.remove(project);
        let last = self.last.get(project).filter(|key| open.contains(key));
        let shown = last.or_else(|| open.first()).cloned();
        self.awaiting = shown.is_none().then(Instant::now);
        shown
    }

    /// Whether the person went to a project with no tab, and none of it shows yet.
    pub(super) fn awaiting(&self) -> bool {
        self.awaiting.is_some_and(|since| since.elapsed() < AWAIT)
    }

    /// The next tab the app opens is of `project`.
    pub(super) fn opening(&mut self, project: &str) {
        self.opening = Some(project.to_string());
    }

    /// The project of the tab opening now, once.
    pub(super) fn take_opening(&mut self) -> Option<String> {
        self.opening.take()
    }

    /// What of this scope is its window's (the project it shows, each project's last tab in it,
    /// a project it waits on) traded with `other`'s: the window in use is another. The marks and
    /// the project of the tab opening next are the app's, and stay.
    pub(super) fn trade_window(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.shown, &mut other.shown);
        std::mem::swap(&mut self.last, &mut other.last);
        std::mem::swap(&mut self.awaiting, &mut other.awaiting);
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
