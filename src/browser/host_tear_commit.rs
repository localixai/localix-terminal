//! A torn tab committed to a destination window, or returned to its source.

use super::super::super::deck::Carried;
use super::super::super::strip::Key;
use super::super::Heard;
use super::Looping;

impl Looping {
    /// The tab's panes out of window `at`, as they are.
    pub(super) fn take(&mut self, at: usize, keys: &[Key]) -> Vec<Carried> {
        let shared = std::sync::Arc::clone(&self.shared);
        let Some(deck) = self.windows.deck_mut(at) else {
            return vec![];
        };
        keys.iter()
            .filter_map(|key| deck.take(*key, &shared))
            .collect()
    }

    /// The tab let go: into the list it is over, or, over none, its window stays where it is.
    pub(super) fn let_go(&mut self) -> bool {
        let Some(tear) = self.tear.take() else {
            return false;
        };
        if let Some(deck) = self.windows.deck(tear.moving) {
            deck.see_through(false);
        }
        let Some(name) = tear.keys.first().map(|key| key.name()) else {
            return true;
        };
        let Some(into) = tear.over else {
            // Over no list: a window a tab came out into stays, a window of its own; the layout
            // opens it, and the app names it as it is told.
            if tear.loose {
                self.windows.keep_loose();
                self.shared.heard(&name, Heard::Detached);
            }
            return true;
        };
        if let Some(deck) = self.windows.deck(into) {
            deck.tell_side("DS()");
        }
        let carried = self.take(tear.moving, &tear.keys);
        let shared = std::sync::Arc::clone(&self.shared);
        if let Some(deck) = self.windows.deck_mut(into) {
            for one in carried {
                deck.take_in(one, &shared);
            }
            deck.layout();
        }
        let window = self.windows.id_at(into);
        let moved = Heard::Moved {
            before: tear.before.as_deref(),
            window: Some(window),
        };
        self.shared.heard(&name, moved);
        if tear.loose {
            self.windows.remove(tear.moving, &self.shared);
        } else if tear.from != 0 {
            // The window that moved itself had that tab only: it is gone with it.
            let gone = self.windows.id_at(tear.from);
            self.windows.remove(tear.from, &self.shared);
            self.shared.heard("", Heard::WindowClosed(gone));
        }
        let into = if tear.loose || tear.from == 0 || into < tear.from {
            into
        } else {
            into - 1
        };
        self.windows.use_at(into, &self.shared);
        self.stale = true;
        true
    }

    /// The pointer is back over the source's list: a window the tab came out into gives it back
    /// and closes; one that moved itself stays where it is.
    pub(super) fn take_back(&mut self) -> bool {
        let Some(tear) = self.tear.take() else {
            return false;
        };
        if let Some(deck) = tear.over.and_then(|at| self.windows.deck(at)) {
            deck.tell_side("DS()");
        }
        if let Some(deck) = self.windows.deck(tear.moving) {
            deck.see_through(false);
        }
        // Back in its list: the card follows the pointer again.
        self.card(true);
        if !tear.loose {
            return true;
        }
        let carried = self.take(tear.moving, &tear.keys);
        let shared = std::sync::Arc::clone(&self.shared);
        if let Some(deck) = self.windows.deck_mut(tear.from) {
            for one in carried {
                deck.take_in(one, &shared);
            }
            deck.layout();
            deck.refresh(&shared);
        }
        self.windows.remove(tear.moving, &self.shared);
        true
    }
}
