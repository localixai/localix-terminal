//! The tree of files in this window's side: which project's it shows, and its page told of it.
//! Whole when the tree comes into sight or the side's page was loaded again, and after that
//! only the news the app sends.

use std::hash::{Hash, Hasher};

use super::super::host::Shared;
use super::super::session;
use super::super::strip::Key;
use super::super::told::{Heard, TreeDid};
use super::super::{file_icons, tree};
use super::{Deck, LOADS, Over, Part};

impl Deck {
    /// This window, among those that show a tree.
    pub(in super::super) fn tree_window(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.frame.native.id().hash(&mut hasher);
        hasher.finish()
    }

    /// The project whose tree this window shows: the one shown, while the side is open and
    /// the person has the files in it.
    fn tree_sight(&self, shared: &Shared) -> Option<String> {
        let files = crate::lock(&shared.kept).files.unwrap_or(false);
        (self.side.is_some() && self.side_open && files)
            .then(|| self.shown_project(shared))
            .filter(|project| !project.is_empty())
    }

    /// The side told of its tree: whether the files show, and the tree of the project shown,
    /// whole where its page holds none of it, else `news` (a project's key, the call that draws
    /// what changed) when it is of that tree. The app hears of a tree that came into sight or
    /// went out of every window's.
    pub(in super::super) fn tell_tree(&self, shared: &Shared, news: Option<(&str, &str)>) {
        let Some(side) = self.side.as_ref() else {
            return;
        };
        let files = crate::lock(&shared.kept).files.unwrap_or(false);
        self.say(
            (Part::Side, side),
            "files",
            if files { "FO(true)" } else { "FO(false)" },
        );
        let (window, sight) = (self.tree_window(), self.tree_sight(shared));
        let changed = crate::lock(&shared.trees).sight(window, sight.as_deref());
        // Told with no lock held: the app's listener runs its own course.
        for (project, seen) in changed {
            shared.heard(&project, Heard::Tree(TreeDid::Seen(seen)));
        }
        let loads = LOADS.load(std::sync::atomic::Ordering::SeqCst);
        let mut trees = crate::lock(&shared.trees);
        let mut told = trees.told(window).clone();
        let Some(project) = sight else {
            if told.tree.take().is_some() {
                let _ = side.evaluate_script(tree::NONE);
            }
            *trees.told(window) = told;
            return;
        };
        if told.pictures != Some(loads) {
            let _ = side.evaluate_script(file_icons::script());
            told.pictures = Some(loads);
        }
        let now = Some((project.clone(), loads));
        if told.tree == now {
            if let Some((of, script)) = news
                && of == project
            {
                let _ = side.evaluate_script(script);
            }
        } else if let Some(whole) = trees.whole(&project) {
            let _ = side.evaluate_script(&whole);
            told.tree = now;
        } else if told.tree.take().is_some() {
            // Another project's tree, not read yet: the one before it goes now.
            let _ = side.evaluate_script(tree::NONE);
        }
        *trees.told(window) = told;
    }

    /// The system's menu of `entries` in the side, at `at` of it; the place of the item picked.
    pub(in super::super) fn pick_in_side(
        &self,
        at: (f64, f64),
        entries: &[super::super::choice::Entry],
    ) -> Option<usize> {
        super::super::platform::show(self.side.as_ref()?, at, entries)
    }

    /// The terminal of the session this window shows, by its key: the tab shown when it is
    /// one, else the terminal its document is held beside. Where a path given from the tree
    /// is typed.
    pub(in super::super) fn session_terminal(&self, shared: &Shared) -> Option<String> {
        let shown = self.shown_key()?;
        if shown.kind() == "term" {
            return Some(shown.name());
        }
        let arranged = shared.arranged_of(&self.shown_project(shared))?;
        let shown = shown.name();
        let lead = session::led(&arranged, Some(&shown)).remove(&shown)?;
        (Key::parse(&lead).filter(|lead| lead.kind() == "term")).map(Key::name)
    }

    /// The pane at `x`, `y` of the window (points from its top left): a split window's half
    /// there, a whole one's tab; none off the pages.
    pub(in super::super) fn pane_under(&self, x: f64, y: f64) -> Option<Key> {
        if matches!(self.over(x, y), Over::Off) {
            return None;
        }
        self.half_at(x, y).or_else(|| self.shown_key())
    }
}
