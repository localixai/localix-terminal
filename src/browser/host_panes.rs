//! The keyboard on the panes of the tab shown (the Pane menu, docs/windows.md): a new terminal
//! beside the pane in use, the keys to the pane a way from it or to the next, the pane in use
//! zoomed and back, a divider moved a step, and the tab made even. Each is drawn at once where
//! the window can (deck_keys.rs) and told to the layout as one operation; a split that would
//! not fit is not asked for, as its greyed menu item says.

use super::super::On;
use super::super::clicks::act;
use crate::browser::split::Own;
use crate::browser::strip::Click;
use crate::browser::{Asked, Heard, Side};
use crate::layout::Tree;

impl On<'_> {
    /// A new pane on `side` of the pane in use (`asked`: a terminal, or an agent's program in
    /// one), in its tab: the desk opens it in the folder of the terminal beside, and it takes
    /// the keys. Nothing where it would not keep its least (geometry's `fits`).
    pub(in super::super) fn split_new(&mut self, side: Side, asked: Asked) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let (Some(shown), true) = (open.shown_key(), open.fits(side)) else {
            return;
        };
        let group = open.shown_project(self.shared);
        let beside = shown.name();
        let split = Heard::SplitNew {
            beside: &beside,
            side,
            asked,
        };
        self.shared.heard(&group, split);
    }

    /// The keys to the pane on `side` of the one in use (`Some`), or to the next of the tab's
    /// panes (`None`, `forward` or back).
    pub(in super::super) fn pane_to(&mut self, side: Option<Side>, forward: bool) {
        // A way to a pane ends a zoom; the next and the one before take it along.
        if side.is_some() {
            self.unzoom();
        }
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let next = match side {
            Some(side) => open.toward(side),
            None => open.pane_after(forward),
        };
        if let Some(key) = next
            && act(self.deck, Click::Show(key), self.shared).is_ok()
            && let Some(open) = self.deck.as_ref()
        {
            open.focus_shown();
        }
    }

    /// The divider beside the pane in use a step towards `side` (`far`: a longer one), told
    /// as a divider let go of is.
    pub(in super::super) fn resize_pane(&mut self, side: Side, far: bool) {
        self.unzoom();
        let held = (self.deck.as_mut()).and_then(|open| open.step_divider(side, far));
        self.split_sized(held);
    }

    /// The tab shown made even: each divider drawn where it goes at once, the layout told.
    pub(in super::super) fn equalize(&mut self) {
        self.unzoom();
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let Some(shown) = open.shown_key().filter(|_| open.even_all()) else {
            return;
        };
        self.shared.heard(&shown.name(), Heard::Equalized);
    }

    /// Zoom Pane (⇧⌘↩): the pane in use alone over its tab's part, the rest put by and
    /// running; zoomed, the tab drawn whole again. Drawn at once and told to the layout.
    /// Nothing for a pane alone in its tab.
    pub(in super::super) fn zoom(&mut self) {
        match self.deck.as_ref().and_then(|open| open.zoomed()) {
            Some(true) => {
                self.unzoom();
            }
            Some(false) => self.zoom_in(),
            None => {}
        }
    }

    /// The pane in use alone over its tab, drawn at once (as a pane the person made whole is,
    /// split.rs `Own`) and told.
    fn zoom_in(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let Some(shown) = open.shown_key() else {
            return;
        };
        open.split.own = Some(Own {
            tree: Tree::leaf(shown.name()),
            base: open.base(),
        });
        self.shared.heard(&shown.name(), Heard::Zoomed(true));
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// A zoomed tab drawn whole again at once, and told: what ends a zoom (a way to a pane, a
    /// divider moved, the tab made even) then acts on the tree drawn. Whether it was zoomed.
    pub(super) fn unzoom(&mut self) -> bool {
        let Some(open) = self.deck.as_mut() else {
            return false;
        };
        let (Some(true), Some(shown), Some(tree)) =
            (open.zoomed(), open.shown_key(), open.tab_tree())
        else {
            return false;
        };
        open.split.own = Some(Own {
            tree,
            base: open.base(),
        });
        self.shared.heard(&shown.name(), Heard::Zoomed(false));
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
        true
    }
}
