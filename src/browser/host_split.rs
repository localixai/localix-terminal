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
        // Laid out where it landed as it was carried: so it stays.
        let over = open.over(x, y);
        if open.land() {
            return open.refresh(self.shared);
        }
        let crate::browser::deck::Over::Half(right) = over else {
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

    /// The window's split and its project's as the layout keeps it, made one: the person split
    /// it (or swapped, or resized), and the layout is told; the person shows one of the kept
    /// split's two tabs, and the window splits as it was. A half's tab closed, the layout is
    /// told the split is gone. Each change is told once, and nothing told is undone before the
    /// layout tells it back.
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
        let kept = (self.shared.arranged_of(&group).as_ref()).and_then(split::kept_of);
        let now = (open.pair().and(open.split.beside)).map(|beside| split::Kept {
            shown: id.clone(),
            beside: beside.key.name(),
            right: beside.right,
        });
        let told = open.split.told.clone();
        let tell = match (now, kept) {
            (Some(now), kept) if kept.as_ref() != Some(&now) && !same(&now, kept.as_ref()) => {
                (told != split::Told::of(Some(&now))).then_some(Some(now))
            }
            (Some(_), _) | (None, None) => None,
            (None, Some(kept)) => {
                let there = |name: &str| group_of(name) == Some(group.as_str());
                if !there(&kept.shown) || !there(&kept.beside) {
                    (told != split::Told::Whole).then_some(None)
                } else {
                    // Undone by the person and not yet told back: it stays undone.
                    if told != split::Told::Whole
                        && let Some(beside) = split::again(&kept, &id)
                    {
                        open.split.beside = Some(beside);
                    }
                    None
                }
            }
        };
        if let Some(tell) = tell {
            open.split.told = split::Told::of(tell.as_ref());
            self.tell_split(&group, tell.as_ref());
        }
    }

    /// The person made the window whole: the layout is told.
    fn unsplit(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        open.split_with(None);
        let group = open.shown_project(self.shared);
        let kept = (self.shared.arranged_of(&group).as_ref()).and_then(split::kept_of);
        if kept.is_some() {
            open.split.told = split::Told::Whole;
            self.tell_split(&group, None);
        }
    }

    /// The layout told the split of project `group` (`kept`), or that it has none.
    fn tell_split(&self, group: &str, kept: Option<&split::Kept>) {
        let halves = kept.map(|kept| Halves {
            shown: &kept.shown,
            beside: &kept.beside,
            right: kept.right,
        });
        self.shared.heard(group, Heard::Split(halves));
    }

    /// The layout of the window's projects, as the app keeps it: the order of their tabs and
    /// their splits. Kept as told; the window draws it, splitting as kept when it shows one of
    /// a split's tabs.
    pub(super) fn arranged(&mut self, arranged: Vec<crate::browser::Arranged>) {
        {
            let mut kept = crate::lock(&self.shared.arranged);
            kept.clear();
            for one in arranged {
                kept.insert((one.window, one.group.clone()), one);
            }
        }
        self.arranged_here();
    }

    /// This window's layout as last told, drawn: what it told the layout is heard back.
    pub(super) fn arranged_here(&mut self) {
        if let Some(open) = self.deck.as_mut() {
            open.split.told = split::Told::Nothing;
        }
        self.keep_split();
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
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

/// Whether split `now` is split `kept` seen from its other half: the same two tabs on the same
/// sides, the other one in use.
fn same(now: &split::Kept, kept: Option<&split::Kept>) -> bool {
    kept.is_some_and(|kept| split::sides(kept) == split::sides(now))
}
