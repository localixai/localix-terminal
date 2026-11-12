//! A split window, from the person's side: the window splits (its tab shown beside another of
//! its project's) or is whole again, and a press in the half not in use makes it the one in use.

use super::On;
use super::clicks::act;
use crate::browser::deck::Deck;
use crate::browser::session;
use crate::browser::split;
use crate::browser::strip::{Click, Key};
use crate::browser::{Halves, Heard};

/// What the window tells the layout of its split.
enum Tell {
    /// It is split so now.
    Split(split::Kept),
    /// The split the layout kept, of these two, is whole now.
    Whole(split::Kept),
}

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
        let Some(key) = Key::parse(&shown.id) else {
            return;
        };
        let arranged = self.shared.arranged_of(&shown.group);
        // A session kept whole shows its document again; else the next tab the layout takes
        // beside this one.
        let back = (arranged.as_ref())
            .and_then(|arranged| session::beside_whole(arranged, &shown.id))
            .and_then(|kept| split::again(&kept, &shown.id));
        let takes = |other: &Key| {
            (arranged.as_ref()).is_none_or(|arranged| session::takes(arranged, key, *other))
        };
        let tabs: Vec<Key> = (listed.iter())
            .filter(|tab| tab.group == shown.group)
            .filter_map(|tab| Key::parse(&tab.id))
            .filter(|other| *other == key || takes(other))
            .collect();
        match (back, split::partner(&tabs, key)) {
            (Some(beside), _) => {
                open.split.beside = Some(beside);
                open.split.grabbed = None;
            }
            (None, Some(partner)) => open.split_with(Some(partner)),
            (None, None) => return,
        }
        open.refresh(self.shared);
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
        let arranged = self.shared.arranged_of(&group);
        // A tab of another project, or one the layout does not take beside this one (a session
        // keeps its documents and takes no other pane), is not beside it.
        let astray = beside.is_some_and(|beside| {
            group_of(&beside).is_some_and(|of| !of.is_empty() && of != group)
        });
        let refused = match (&arranged, Key::parse(&id), open.split.beside) {
            (Some(arranged), Some(key), Some(beside)) => !session::takes(arranged, key, beside.key),
            _ => false,
        };
        if astray || refused {
            open.split_with(None);
        }
        let split_of = |arranged: &crate::browser::Arranged| session::split_of(arranged, &id);
        let kept = arranged.as_ref().and_then(split_of);
        // A session's panes are the layout's to keep: it hears of one closed by itself.
        let session = (arranged.as_ref())
            .and_then(|arranged| session::tab_of(arranged, &id))
            .is_some_and(|tab| !tab.docs.is_empty());
        let split_now = |open: &Deck| {
            (open.pair().and(open.split.beside)).map(|beside| split::Kept {
                shown: id.clone(),
                beside: beside.key.name(),
                right: beside.right,
            })
        };
        // The layout keeps another split than the one this window drew from it: the app put a
        // pane beside the tab shown, or took one away, and the window follows.
        let drawn = (open.split.drawn.take()).filter(|(of, _)| *of == group);
        if let Some((_, drawn)) = drawn {
            let now = split_now(open);
            match split::followed(now.as_ref(), drawn.as_ref(), kept.as_ref(), &id) {
                split::Follow::Stay => {}
                split::Follow::Beside(beside) => {
                    open.split.beside = Some(beside);
                    open.split.grabbed = None;
                }
                split::Follow::Whole => open.split_with(None),
            }
        }
        open.split.drawn = Some((group.clone(), kept.clone()));
        let now = split_now(open);
        let told = open.split.told.clone();
        let tell = match (now, kept) {
            (Some(now), kept)
                if kept.as_ref() != Some(&now) && !split::same(&now, kept.as_ref()) =>
            {
                (told != split::Told::of(Some(&now))).then_some(Tell::Split(now))
            }
            (Some(_), _) | (None, None) => None,
            (None, Some(kept)) => {
                let there = |name: &str| group_of(name) == Some(group.as_str());
                if !there(&kept.shown) || !there(&kept.beside) {
                    (told != split::Told::Whole && !session).then_some(Tell::Whole(kept))
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
        let pair = open.pair();
        let saved = arranged
            .as_ref()
            .and_then(|arranged| split::share_of(arranged, pair?));
        open.split.follow_share(pair, saved);
        if let Some(tell) = tell {
            open.split.told = match &tell {
                Tell::Split(now) => split::Told::of(Some(now)),
                Tell::Whole(_) => split::Told::Whole,
            };
            self.tell_split(&group, &tell);
        }
    }

    /// Persist only the final size, never a write per pointer frame. Until projected back,
    /// this pair keeps its new size instead of following an older layout snapshot.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the rounded share is explicitly bounded to 1000..=9000"
    )]
    pub(super) fn split_sized(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let Some((left, right)) = open.pair() else {
            return;
        };
        let Some(key) = open.shown_key() else { return };
        let portion = (open.split.share * 10_000.0).round().clamp(1000.0, 9000.0) as u16;
        open.split.sizing = Some(([left, right], f64::from(portion) / 10_000.0));
        self.shared.heard(&key.name(), Heard::SplitResized(portion));
    }

    /// The person made the window whole: the layout is told which split it was.
    fn unsplit(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let shown = open.shown_key().map(Key::name).unwrap_or_default();
        open.split_with(None);
        let group = open.shown_project(self.shared);
        let kept = (self.shared.arranged_of(&group).as_ref())
            .and_then(|arranged| session::split_of(arranged, &shown));
        if let Some(kept) = kept {
            open.split.told = split::Told::Whole;
            self.tell_split(&group, &Tell::Whole(kept));
        }
    }

    /// The layout told of project `group`'s split: the one the window has now, or the one it
    /// kept that is whole now.
    fn tell_split(&self, group: &str, tell: &Tell) {
        let (Tell::Split(kept) | Tell::Whole(kept)) = tell;
        let halves = Halves {
            shown: &kept.shown,
            beside: &kept.beside,
            right: kept.right,
        };
        let said = match tell {
            Tell::Split(_) => Heard::Split(halves),
            Tell::Whole(_) => Heard::Whole(halves),
        };
        self.shared.heard(group, said);
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
