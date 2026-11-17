//! A tab carried out of a window's list, as docs/windows.md has it: past the window's edge it
//! comes out into a window of its own that follows the pointer (with one tab only, its window is
//! what moves); over another window's list that window turns clear and the list opens a slot;
//! let go over a list the tab goes into it, elsewhere its window stays. Nothing is written to
//! the layout before it is let go. The gesture's state is the carry's (`host_carry.rs`).

use tao::event_loop::EventLoopWindowTarget;

use super::super::super::carry::Point;
use super::super::super::deck::Deck;
use super::super::super::strip::Key;
use super::{Cmd, Looping};

/// Where the pointer is in a window a tab came out into: over its list's first row.
const GRAB: (f64, f64) = (72.0, 54.0);

/// A tab out of its window's list, carried by the pointer.
pub(super) struct Tear {
    /// Its panes (a split's two, or one).
    pub(super) keys: Vec<Key>,
    /// The window it came out of.
    pub(super) from: usize,
    /// The window that follows the pointer: a new one, or `from` itself when it had that tab only.
    pub(super) moving: usize,
    /// Whether `moving` is a new window, not in the layout yet.
    pub(super) loose: bool,
    /// The pointer from the moving window's top left.
    grab: (f64, f64),
    /// The window whose list it is over, and the tab of that list's slot (none: its end).
    pub(super) over: Option<usize>,
    pub(super) before: Option<String>,
}

impl Looping {
    /// The carried tab's card seen where the pointer is, or not (a window carries the tab).
    pub(super) fn card(&self, shown: bool) {
        if let Some(carrying) = &self.carry {
            carrying.chip.show(shown);
        }
    }

    /// The tab of `key` carried out past its window's edge, at `x`, `y` of its side's page
    /// (`screen`, the screen's point as the page says it).
    pub(super) fn follow(
        &mut self,
        key: Key,
        (x, y): Point,
        screen: Option<Point>,
        target: &EventLoopWindowTarget<Cmd>,
    ) -> bool {
        let from = (self.carry.as_ref()).map_or_else(|| self.windows.at(), |it| it.from);
        // The screen's point as the page says it: a window that moves under the pointer moves the
        // page's points with it, so they and where the window was disagree.
        let point = self
            .windows
            .deck(from)
            .and_then(|deck| screen.or_else(|| deck.to_screen(x, y + deck.top())));
        point.is_some_and(|point| self.carry_out(key, from, point, target))
    }

    /// The tab carried out of window `from` to `screen`, a point of the screen: it comes out
    /// first, then its window follows, over a list or not.
    fn carry_out(
        &mut self,
        key: Key,
        from: usize,
        screen: Point,
        target: &EventLoopWindowTarget<Cmd>,
    ) -> bool {
        let torn = self
            .carry
            .as_ref()
            .is_some_and(|carrying| carrying.tear.is_some());
        if !torn && !self.come_out(key, from, screen, target) {
            return false;
        }
        let Some(tear) = self
            .carry
            .as_ref()
            .and_then(|carrying| carrying.tear.as_ref())
        else {
            return false;
        };
        let (moving, grab, loose, from, was) =
            (tear.moving, tear.grab, tear.loose, tear.from, tear.over);
        if let Some(deck) = self.windows.deck(moving) {
            deck.move_to(screen.0 - grab.0, screen.1 - grab.1);
        }
        // Over which window's list: not the moving one's, nor (out of it) the source's, whose
        // own list takes the tab back as the side's page says.
        let over = (0..self.windows.count())
            .filter(|at| *at != moving && !(loose && *at == from))
            .find(|at| {
                (self.windows.deck(*at)).is_some_and(|deck| deck.over_list(screen.0, screen.1))
            });
        if was != over {
            if let Some(deck) = was.and_then(|at| self.windows.deck(at)) {
                deck.tell_side("DS()");
            }
            if let Some(deck) = self.windows.deck(moving) {
                deck.see_through(over.is_some());
            }
            // Over a list the tab is a card again, where the pointer holds it; elsewhere the
            // window it is in is what follows the pointer.
            self.card(over.is_some());
            if let Some(deck) = over.and_then(|at| self.windows.deck(at)) {
                deck.to_front();
            }
            if let Some(tear) = self
                .carry
                .as_mut()
                .and_then(|carrying| carrying.tear.as_mut())
            {
                tear.over = over;
                tear.before = None;
            }
        }
        if let Some(deck) = over.and_then(|at| self.windows.deck(at)) {
            let tall = (self.carry.as_ref()).map_or(44.0, |carrying| carrying.chip.height());
            deck.tell_side(&format!(
                "DS({},{})",
                deck.list_y(screen.1).round(),
                tall.round()
            ));
        }
        true
    }

    /// The tab of `key` comes out of window `from` at `screen`: into a new window that follows
    /// the pointer, its views moved into it as they are, or, when it is the only tab the
    /// window shows, the window itself follows.
    fn come_out(
        &mut self,
        key: Key,
        from: usize,
        screen: (f64, f64),
        target: &EventLoopWindowTarget<Cmd>,
    ) -> bool {
        let group = crate::lock(&self.shared.scope).shown().to_string();
        let name = key.name();
        let arranged = self.shared.arranged_here();
        let tabs = arranged
            .get(&group)
            .map(|arranged| arranged.tabs.clone())
            .unwrap_or_default();
        let keys: Vec<Key> = (tabs.iter())
            .find(|tab| tab.all.contains(&name))
            .map(|tab| tab.all.iter().filter_map(|pane| Key::parse(pane)).collect())
            .unwrap_or_else(|| vec![key]);
        let Some(source) = self.windows.deck(from) else {
            return false;
        };
        // The window moves itself only when the tab holds every pane it has: its other projects'
        // tabs, hidden and ended terminals and Settings are panes of it too, and go with it.
        let alone = source.keys().iter().all(|one| keys.contains(one));
        if alone {
            // The window moves under the pointer, its list with it: its page is told the tab is
            // out of it till it is let go, wherever the pointer is.
            source.tell_side("TO()");
            self.card(false);
            let tear = Some(Tear {
                keys,
                from,
                moving: from,
                loose: false,
                grab: GRAB,
                over: None,
                before: None,
            });
            if let Some(carrying) = self.carry.as_mut() {
                carrying.tear = tear;
            }
            return true;
        }
        let mut deck = match Deck::empty((target, 1), &self.shared, &self.proxy) {
            Ok(deck) => deck,
            Err(why) => {
                log::warn!("no window for the tab carried out: {why}");
                return false;
            }
        };
        let carried = self.take(from, &keys);
        if carried.is_empty() {
            return false;
        }
        for one in carried {
            deck.take_in(one, &self.shared);
        }
        deck.move_to(screen.0 - GRAB.0, screen.1 - GRAB.1);
        deck.layout();
        deck.refresh(&self.shared);
        deck.reveal();
        if let Some(source) = self.windows.deck(from) {
            source.layout();
            source.refresh(&self.shared);
        }
        self.card(false);
        let moving = self.windows.add_loose(deck, &self.shared);
        let tear = Some(Tear {
            keys,
            from,
            moving,
            loose: true,
            grab: GRAB,
            over: None,
            before: None,
        });
        if let Some(carrying) = self.carry.as_mut() {
            carrying.tear = tear;
        }
        true
    }
}
