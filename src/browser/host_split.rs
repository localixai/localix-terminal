//! A split window, from the person's side: the window splits (its tab shown beside another of
//! its project's) or is whole again, and a press in the half not in use makes it the one in use.

use super::On;
use super::clicks::act;
use crate::browser::split;
use crate::browser::strip::{Click, Key};
use crate::browser::{Halves, Heard};

impl On<'_> {
    /// Split the window, the tab shown beside the next of its project's (else the one before),
    /// or make it whole again.
    pub(super) fn split_view(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        if open.pair().is_some() {
            self.unsplit();
            if let Some(open) = self.deck.as_ref() {
                open.refresh(self.shared);
            }
            return;
        }
        let listed = open.listing(self.shared);
        let Some(shown) = listed.iter().find(|tab| tab.shown) else {
            return;
        };
        let tabs: Vec<Key> = (listed.iter())
            .filter(|tab| tab.group == shown.group)
            .filter_map(|tab| Key::parse(&tab.id))
            .collect();
        let partner = Key::parse(&shown.id).and_then(|key| split::partner(&tabs, key));
        if partner.is_some() {
            open.split_with(partner);
            open.refresh(self.shared);
        }
    }

    /// Tab `key` beside the one shown, the window split; or the window whole again (none).
    pub(super) fn split_beside(&mut self, key: Option<Key>) {
        match key {
            Some(_) => {
                if let Some(open) = self.deck.as_mut() {
                    open.split_with(key);
                }
            }
            None => self.unsplit(),
        }
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// Tab `key` let go of over the pages at `x`, `y` of the window. A whole window splits, the
    /// tab beside the shown one in that half. Split, the tab takes that half: the half in use shows
    /// it there, the other has it beside; the shown tab let go of over the other half changes
    /// sides with it.
    pub(super) fn dropped(&mut self, key: Key, x: f64, y: f64) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let target = open.drop_target(x, y);
        open.drop_over(None);
        let Some(right) = target else {
            return;
        };
        let shown = open.shown_key();
        match open.pair() {
            None if Some(key) != shown => {
                open.split.beside = Some(split::Beside { key, right });
                open.split.grabbed = None;
            }
            None => return,
            Some((left, right_half)) => {
                let into = if right { right_half } else { left };
                if Some(into) == shown {
                    // The tab takes the half in use: the one beside stays.
                    let beside = open.split.beside;
                    if Some(key) != shown && beside.is_some_and(|beside| beside.key != key) {
                        let _ = act(self.deck, Click::Show(key), self.shared);
                        if let Some(open) = self.deck.as_mut() {
                            open.split.beside = beside;
                            open.refresh(self.shared);
                        }
                    }
                    return;
                }
                if let Some(beside) = open.split.beside.as_mut() {
                    if Some(key) == shown {
                        beside.right = !beside.right;
                    } else {
                        *beside = split::Beside { key, right };
                    }
                }
            }
        }
        open.refresh(self.shared);
    }

    /// The split kept with its project. Split, the project keeps it, and the app is told.
    /// Another tab shown (of the project, or of another: a split window shows one project), the
    /// window is whole and the project keeps its split; one of its two tabs shown again, the
    /// window splits as it was. A half's tab closed, the project keeps none.
    pub(super) fn keep_split(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let listed = open.listing(self.shared);
        let Some(shown) = listed.iter().find(|tab| tab.shown) else {
            return;
        };
        // The window's own tabs (the settings) are of no project: the split stays as it is.
        if shown.group.is_empty() {
            return;
        }
        let (group, id) = (shown.group.clone(), shown.id.clone());
        let group_of =
            |name: &str| (listed.iter().find(|tab| tab.id == name)).map(|tab| tab.group.as_str());
        let beside = open.split.beside.map(|beside| beside.key.name());
        if beside
            .is_some_and(|beside| group_of(&beside).is_some_and(|of| !of.is_empty() && of != group))
        {
            open.split_with(None);
        }
        let mut splits = crate::lock(&self.shared.splits);
        let now = (open.pair().and(open.split.beside)).map(|beside| split::Kept {
            shown: id.clone(),
            beside: beside.key.name(),
            right: beside.right,
        });
        let told = match (now, splits.get(&group).cloned()) {
            (Some(now), kept) if kept.as_ref() != Some(&now) => {
                splits.insert(group.clone(), now.clone());
                Some(Some(now))
            }
            (Some(_), _) | (None, None) => None,
            (None, Some(kept)) => {
                let there = |name: &str| group_of(name) == Some(group.as_str());
                if !there(&kept.shown) || !there(&kept.beside) {
                    splits.remove(&group);
                    Some(None)
                } else {
                    if let Some(beside) = split::again(&kept, &id) {
                        open.split.beside = Some(beside);
                    }
                    None
                }
            }
        };
        drop(splits);
        if let Some(told) = told {
            self.tell_split(&group, told.as_ref());
        }
    }

    /// The person made the window whole: its project keeps no split.
    fn unsplit(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        open.split_with(None);
        let group = open.shown_project(self.shared);
        if crate::lock(&self.shared.splits).remove(&group).is_some() {
            self.tell_split(&group, None);
        }
    }

    /// The app told project `group`'s split (`kept`), or that it has none.
    fn tell_split(&self, group: &str, kept: Option<&split::Kept>) {
        let halves = kept.map(|kept| Halves {
            shown: &kept.shown,
            beside: &kept.beside,
            right: kept.right,
        });
        self.shared.heard(group, Heard::Split(halves));
    }

    /// The app kept project `project`'s split across a restart, and its tabs are open again:
    /// shown, the window splits as it was; else it splits so once the person comes to it.
    pub(super) fn split_kept(&mut self, project: &str, kept: split::Kept) {
        let halves = (Key::parse(&kept.shown)).zip(split::again(&kept, &kept.shown));
        crate::lock(&self.shared.splits).insert(project.to_string(), kept);
        if crate::lock(&self.shared.scope).shown() != project {
            return;
        }
        let Some((shown, beside)) = halves else {
            return;
        };
        let _ = act(self.deck, Click::Show(shown), self.shared);
        if let Some(open) = self.deck.as_mut()
            && open.shown_key() == Some(shown)
        {
            open.split.beside = Some(beside);
        }
    }

    /// The person pressed `x`, `y` points from the top left of `window` (an NSWindow's address):
    /// in the half of a split window not in use, that half's tab is the one in use now.
    pub(super) fn pressed(&mut self, window: usize, x: f64, y: f64) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        if !open.is_window(window) {
            return;
        }
        let Some(key) = open
            .half_at(x, y)
            .filter(|key| Some(*key) != open.shown_key())
        else {
            return;
        };
        let _ = act(self.deck, Click::Show(key), self.shared);
    }
}
