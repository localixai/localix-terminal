//! The keyboard on the panes of the tab shown (docs/windows.md): which pane lies a way from the
//! one in use, the next and the one before, whether it is zoomed, a divider moved a step, the
//! tab made even, whether a pane splits, and which pane takes the keys when one closes. The panes the person used are
//! kept here, the latest first, only while the window runs: a tie goes to the one used last.

use super::super::Side;
use super::super::split::{GAP, Held, SPACING};
use super::super::strip::Key;
use super::Deck;
use crate::layout::geometry::{self, Laid, Rect, STEP, STEP_FAR};

/// How many panes used the window remembers.
const RECENT: usize = 32;

impl Deck {
    /// Pane `key` is the one in use now: the first of those used.
    pub(in super::super) fn used(&self, key: Key) {
        let mut recent = crate::lock(&self.recent);
        if recent.first() != Some(&key) {
            recent.retain(|other| *other != key);
            recent.insert(0, key);
            recent.truncate(RECENT);
        }
    }

    /// The panes used, the latest first.
    fn recent(&self) -> Vec<Key> {
        crate::lock(&self.recent).clone()
    }

    /// The pane on `side` of the one in use, in the tree shown: of those beside it that way,
    /// the one used last. None at the tab's edge, or with a pane alone.
    pub(in super::super) fn toward(&self, side: Side) -> Option<Key> {
        let (panes, shown) = (self.panes()?, self.shown_key()?);
        geometry::neighbour(&panes.panes, &shown, side, &self.recent(), GAP)
    }

    /// Whether the pane in use is drawn alone over a tab of several (zoomed, or so drawn till
    /// the layout hears it): none for a pane alone in its tab, which has nothing to zoom.
    pub(in super::super) fn zoomed(&self) -> Option<bool> {
        self.tab_tree()?;
        Some(self.sight().len() < 2)
    }

    /// The pane after the one in use in the tree shown (`forward`), or before it, in the order
    /// they lie, round from the last to the first; in a tab zoomed, among all its panes, so the
    /// zoom goes to the next.
    pub(in super::super) fn pane_after(&self, forward: bool) -> Option<Key> {
        let shown = self.shown_key()?;
        let sight = match self.zoomed() {
            Some(true) => (self.tab_tree()?.visible().iter())
                .filter_map(|key| Key::parse(key))
                .collect(),
            _ => self.sight(),
        };
        super::super::split::round(&sight, &shown, forward)
    }

    /// Whether the pane in use splits `side`: as wide (or tall) as two panes that keep their
    /// least and the line between them.
    pub(in super::super) fn fits(&self, side: Side) -> bool {
        let Some(shown) = self.shown_key() else {
            return false;
        };
        let laid = self.panes().unwrap_or_else(|| {
            let (size, left, top) = self.area(false);
            let area = Rect::new(left, top, size.width, size.height);
            Laid {
                panes: vec![(shown, area)],
                dividers: vec![],
            }
        });
        geometry::fits(&laid, &shown, side.across(), SPACING)
    }

    /// The divider beside the pane in use moved a step towards `side` (`far`: a longer one):
    /// the one on that side of it, else the one on its other side, so the arrow's way is the
    /// way a divider goes. Held as a drag let go of is (deck_drop.rs): the hold to tell the
    /// layout. None with no divider that way.
    pub(in super::super) fn step_divider(&mut self, side: Side, far: bool) -> Option<Held> {
        let (tree, shown) = (self.tree()?, self.shown_key()?.name());
        let by = if far { STEP_FAR } else { STEP };
        let (path, at, share) = (tree.stepped(&shown, side, by))
            .or_else(|| tree.stepped(&shown, side.opposite(), -by))?;
        let panes = self.panes()?;
        let place =
            (panes.dividers.iter()).position(|divider| divider.path == path && divider.at == at)?;
        let room = panes.dividers.get(place)?.room;
        let now = super::super::split::share_at(&tree, &path, at)?;
        let held = self.holding(place, |held, _| held.share, (share - now) * room)?;
        self.finish_hold(held, true)
    }

    /// Every divider of the tree shown made even by what the children of its branch hold
    /// (`Tree::equalize`), drawn at once as Home on each would. Whether there was one.
    pub(in super::super) fn even_all(&mut self) -> bool {
        let count = self.panes().map_or(0, |panes| panes.dividers.len());
        (0..count)
            .filter(|at| self.split_even(*at).is_some())
            .count()
            > 0
    }

    /// Whether pane `key` of the tab shown goes out to a tab of its own (Move Pane to New Tab):
    /// it is in sight and no document, and another pane in sight is none either.
    pub(in super::super) fn goes_out(&self, key: Key) -> bool {
        let sight = self.sight();
        let own: Vec<&Key> = (sight.iter())
            .filter(|pane| self.stack_of(**pane).is_none())
            .collect();
        own.len() >= 2 && own.contains(&&key)
    }

    /// The pane that takes the keys once pane `key`, in use in a tab of several (`sight`),
    /// closes (split.rs `taker`).
    pub(in super::super) fn taker(&self, key: Key, sight: &[Key]) -> Option<Key> {
        let names = |keys: &[Key]| keys.iter().map(|key| key.name()).collect::<Vec<_>>();
        let tree = self.tree().or_else(|| self.tab_tree())?;
        let taker =
            super::super::split::taker(&tree, &names(&self.recent()), &key.name(), &names(sight));
        taker.and_then(|taker| Key::parse(&taker))
    }
}
