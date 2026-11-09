//! The app's windows (docs/windows.md): each a deck, the first the main one (its minimize button
//! puts it away). One of them is the window in use, as the system's key window is: what the person does
//! goes to it, and what an agent does goes to the window its pane is in. The loop picks the
//! window an event is for before it acts on it, so the rest of the window's code acts on one.

use tao::window::WindowId;

use super::deck::Deck;
use super::host::Shared;
use super::scope::Scope;

/// One window: its deck (none while it opens) and the project scope it shows, kept here while
/// another window is the one in use (the one in use keeps its scope in `Shared`).
struct Slot {
    /// Its row in the layout; 0 while the app has not named it.
    id: i64,
    deck: Option<Deck>,
    scope: Scope,
}

/// The row of a window a tab was carried out into, until the layout names it (docs/windows.md).
const LOOSE: i64 = -1;

pub(super) struct Windows {
    slots: Vec<Slot>,
    /// The window in use.
    current: usize,
    /// A window a tab was carried out into stays: the next window the layout opens is it.
    keep_loose: bool,
}

impl Windows {
    /// The main window, not open yet.
    pub(super) fn new() -> Self {
        Self {
            slots: vec![Slot {
                id: 0,
                deck: None,
                scope: Scope::default(),
            }],
            current: 0,
            keep_loose: false,
        }
    }

    /// The window in use.
    pub(super) fn current(&mut self) -> &mut Option<Deck> {
        &mut self.slots[self.current].deck
    }

    /// The main window.
    pub(super) fn main(&mut self) -> &mut Option<Deck> {
        &mut self.slots[0].deck
    }

    /// Whether the window in use is the main one.
    pub(super) fn in_main(&self) -> bool {
        self.current == 0
    }

    /// Every open window.
    pub(super) fn all(&self) -> impl Iterator<Item = &Deck> {
        self.slots.iter().filter_map(|slot| slot.deck.as_ref())
    }

    /// The window `found` finds, made the one in use: whether there was one.
    fn focus_by(&mut self, shared: &Shared, found: impl Fn(&Deck) -> bool) -> bool {
        let at = (self.slots.iter()).position(|slot| slot.deck.as_ref().is_some_and(&found));
        match at {
            Some(at) => {
                self.focus(at, shared);
                true
            }
            None => false,
        }
    }

    /// The window of native window `id` is the one in use.
    pub(super) fn focus_native(&mut self, id: WindowId, shared: &Shared) -> bool {
        self.focus_by(shared, |deck| deck.frame.native.id() == id)
    }

    /// The window that is NSWindow `window` (its address) is the one in use.
    pub(super) fn focus_ns(&mut self, window: usize, shared: &Shared) -> bool {
        self.focus_by(shared, |deck| deck.is_window(window))
    }

    /// The window pane `key` (`w1`, `t2`) is in is the one in use.
    pub(super) fn focus_key(&mut self, key: &str, shared: &Shared) -> bool {
        self.focus_by(shared, |deck| deck.has(key))
    }

    /// A new window, the one in use from now on: its deck opens with the next tab it is given.
    /// It shows the project the window in use showed.
    pub(super) fn open(&mut self, id: i64, shared: &Shared) {
        // Told twice, it is the one window.
        if let Some(at) = self.slots.iter().position(|slot| slot.id == id) {
            return self.focus(at, shared);
        }
        // The window a tab was carried out into, kept as a window of its own: this is its row.
        if self.keep_loose
            && let Some(at) = self.slots.iter().position(|slot| slot.id == LOOSE)
        {
            self.keep_loose = false;
            self.slots[at].id = id;
            return self.focus(at, shared);
        }
        let scope = crate::lock(&shared.scope).fresh();
        self.slots.push(Slot {
            id,
            deck: None,
            scope,
        });
        let at = self.slots.len() - 1;
        self.focus(at, shared);
    }

    /// The window in use is gone (not the main one, which stays): the main one is in use again.
    pub(super) fn close_current(&mut self, shared: &Shared) {
        self.remove(self.current, shared);
    }

    /// Window `at` is the one in use: its scope is the one `Shared` keeps.
    fn focus(&mut self, at: usize, shared: &Shared) {
        if at == self.current {
            return;
        }
        let mut scope = crate::lock(&shared.scope);
        scope.trade_window(&mut self.slots[self.current].scope);
        scope.trade_window(&mut self.slots[at].scope);
        drop(scope);
        self.current = at;
        (shared.in_use).store(self.slots[at].id, std::sync::atomic::Ordering::SeqCst);
    }

    /// The windows the app keeps open, the main one first: the main window is the first of them.
    pub(super) fn name(&mut self, ids: &[i64], shared: &Shared) {
        if let Some(main) = ids.first()
            && self.slots[0].id == 0
        {
            self.slots[0].id = *main;
            if self.current == 0 {
                (shared.in_use).store(*main, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

/// Which window a command is for.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Route {
    /// The window pane `key` is in (none of them: the window in use).
    Pane(String),
    /// The window the person pressed in, the one in use from now on.
    Pressed(usize),
    /// The window in use; then each other one is drawn again (the app's tabs, titles, styles or
    /// layout changed for all of them).
    All,
    /// The window in use.
    Current,
}

impl Windows {
    /// The window in use's row in the layout (0: not named yet).
    pub(super) fn id(&self) -> i64 {
        self.slots[self.current].id
    }

    /// Which window is in use, to come back to it.
    pub(super) fn at(&self) -> usize {
        self.current
    }

    /// How many windows there are.
    pub(super) fn count(&self) -> usize {
        self.slots.len()
    }

    /// Window `at`'s deck, when it has one.
    pub(super) fn deck(&self, at: usize) -> Option<&Deck> {
        self.slots.get(at).and_then(|slot| slot.deck.as_ref())
    }

    /// Window `at`'s deck, to change.
    pub(super) fn deck_mut(&mut self, at: usize) -> Option<&mut Deck> {
        self.slots.get_mut(at).and_then(|slot| slot.deck.as_mut())
    }

    /// Window `at`'s row in the layout.
    pub(super) fn id_at(&self, at: usize) -> i64 {
        self.slots.get(at).map_or(0, |slot| slot.id)
    }

    /// A window a tab was carried out into: the layout has no row for it yet. It shows the
    /// project the window in use shows, and is not the one in use.
    pub(super) fn add_loose(&mut self, deck: Deck, shared: &Shared) -> usize {
        let scope = crate::lock(&shared.scope).fresh();
        self.slots.push(Slot {
            id: LOOSE,
            deck: Some(deck),
            scope,
        });
        self.slots.len() - 1
    }

    /// The window a tab was carried out into stays, a window of its own: the layout names it.
    pub(super) fn keep_loose(&mut self) {
        self.keep_loose = true;
    }

    /// Window `at` is gone (never the main one): its deck closes with it.
    pub(super) fn remove(&mut self, at: usize, shared: &Shared) {
        if at == 0 || at >= self.slots.len() {
            return;
        }
        if self.current == at {
            self.focus(0, shared);
        }
        self.slots.remove(at);
        if self.current > at {
            self.current -= 1;
        }
    }

    /// Window `at` is the one in use from now on (and again, as it was before a command for
    /// another, or for a while, drawing each window in turn).
    pub(super) fn use_at(&mut self, at: usize, shared: &Shared) {
        if at < self.slots.len() {
            self.focus(at, shared);
        }
    }
}
