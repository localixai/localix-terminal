//! A split tab, from the person's side: the window splits (its tab shown beside another of its
//! project's, or a tab carried onto the pages beside the pane it is let go over, on the side of
//! the edge it is nearest), a pane goes out to a tab of its own, a divider is let go of, and a
//! press on a pane puts the keys there. Each is drawn at once (split.rs) and told to the layout
//! as one operation; its notice brings the tree back. The keyboard's are host_panes.rs.

use super::On;
use super::clicks::act;
use crate::browser::split::{self, Held, Own};
use crate::browser::strip::{Click, Key};
use crate::browser::{Heard, Side};
use crate::layout::Tree;

#[path = "host_panes.rs"]
mod panes;

impl On<'_> {
    /// Split View: in a tab of several panes, the pane in use goes out to a tab of its own (a
    /// session of one terminal is made whole); else the tab shown beside the next of its
    /// project's (else the one before), or a tab zoomed (a session made whole) drawn whole.
    pub(super) fn split_view(&mut self) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        if open.sight().len() >= 2 {
            if let Some(shown) = open.shown_key() {
                self.pane_out(shown);
            }
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
        // A tab zoomed (a session made whole among them) is drawn whole again; else the next
        // tab the layout takes beside this one.
        if self.unzoom() {
            return;
        }
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let tabs: Vec<Key> = (listed.iter())
            .filter(|tab| tab.group == shown.group)
            .filter_map(|tab| Key::parse(&tab.id))
            .filter(|other| *other == key || open.takes(key, *other))
            .collect();
        let Some(partner) = split::partner(&tabs, key) else {
            return;
        };
        self.split_with(key, partner, Side::Right);
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// Tab `key` beside the one shown, the window split; or, none, the pane `out` goes out to a
    /// tab of its own (Move Pane to New Tab).
    pub(super) fn split_beside(&mut self, key: Option<Key>, out: Key) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        match (key, open.shown_key()) {
            (Some(key), Some(shown)) => self.split_with(shown, key, Side::Right),
            _ => self.pane_out(out),
        }
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// Tab `key` put on `side` of pane `beside` of the tab shown: drawn at once, and the layout
    /// told.
    fn split_with(&mut self, beside: Key, key: Key, side: Side) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let was = open.split.own.clone();
        let over = crate::browser::deck::Over::Pane(beside, side);
        let Some(landed) = open.landed(key, over, was.as_ref()) else {
            return;
        };
        self.tell_landed(key, &landed);
    }

    /// Where tab `key` landed drawn at once, and told: `key` on its side of the pane beside.
    fn tell_landed(&mut self, key: Key, landed: &crate::browser::deck::Landed) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        open.split.own = Some(Own {
            tree: landed.tree.clone(),
            base: open.base(),
        });
        let beside = landed.beside.name();
        let side = landed.side;
        let told = Heard::Split {
            beside: &beside,
            side,
        };
        self.shared.heard(&key.name(), told);
    }

    /// Tab `key` let go of over the pages at `x`, `y` of the window: beside the pane it was let
    /// go over, on the side of it it was over, as it was laid out while it was carried.
    pub(super) fn dropped(&mut self, key: Key, x: f64, y: f64) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let over = open.over(x, y);
        let was = open.land();
        match open.landed(key, over, was.as_ref()) {
            Some(landed) => self.tell_landed(key, &landed),
            // Let go where nothing changes: the tab shown as the person had made it.
            None => open.split.own = was,
        }
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// Pane `key` out of the tab it shares: drawn at once (the tab shown without it, or it
    /// alone when it is the one in use), and the layout told.
    pub(super) fn pane_out(&mut self, key: Key) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let Some(shown) = open.shown_key() else {
            return;
        };
        let tree = match key == shown {
            true => Some(Tree::leaf(shown.name())),
            false => open.tree().and_then(|tree| tree.without(&key.name())),
        };
        if let Some(tree) = tree {
            open.split.own = Some(Own {
                tree,
                base: open.base(),
            });
        }
        self.shared.heard(&key.name(), Heard::Whole);
    }

    /// The window's tree made one with the layout's as it tells it now: what the person did
    /// that the layout has heard goes (split.rs `follow`); the pane they put the keys in, one
    /// of a tab of several (zoomed or not), is told as the tab's focus; and a divider's view is made for each
    /// divider the tree has.
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
        let id = shown.id.clone();
        open.keep_stacks(&self.shared.arranged_here());
        let tab = open.tab_holding(&id);
        open.split.follow(tab.as_ref(), &id);
        // A pane of a tab zoomed shown is the tab's focus too: the zoom goes to it.
        let focus = tab.filter(|tab| {
            tab.tree.visible().len() >= 2 && tab.focused.as_deref() != Some(id.as_str())
        });
        if focus.is_some() && open.split.focus.as_deref() != Some(id.as_str()) {
            open.split.focus = Some(id.clone());
            self.shared.heard(&id, Heard::Focused);
        }
        open.lines_enough();
    }

    /// The person let go of a divider (`held`): only its final share is told, never one per
    /// pointer frame; until the layout tells it back, the tree keeps it (split.rs).
    pub(super) fn split_sized(&mut self, held: Option<Held>) {
        let (Some(open), Some(held)) = (self.deck.as_ref(), held) else {
            return;
        };
        let Some(key) = open.shown_key() else { return };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the rounded share is bounded to 1..=9999 first"
        )]
        let share = (held.share * 10_000.0).round().clamp(1.0, 9_999.0) as u16;
        let resized = Heard::Resized {
            path: &held.path,
            at: held.at,
            share,
        };
        self.shared.heard(&key.name(), resized);
    }

    /// The layout of the window's projects, as the app keeps it: the order of their tabs and
    /// their trees. Kept as told; the window draws the tree of the tab its shown pane is in.
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

    /// This window's layout as last told, drawn: what the person did is heard back.
    pub(super) fn arranged_here(&mut self) {
        self.keep_split();
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// The person pressed `x`, `y` points from the top left of `window` (an NSWindow's address):
    /// on a pane of a split tab not in use, that pane is the one in use now.
    pub(super) fn pressed(&mut self, window: usize, x: f64, y: f64) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        if !open.is_window(window) {
            return;
        }
        let Some(key) = open
            .pane_at(x, y)
            .filter(|key| Some(*key) != open.shown_key())
        else {
            return;
        };
        let _ = act(self.deck, Click::Show(key), self.shared);
    }
}
