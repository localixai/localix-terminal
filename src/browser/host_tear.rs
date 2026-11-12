//! A tab carried out of a window's list, as docs/windows.md has it: past the window's edge it
//! comes out into a window of its own that follows the pointer (with one tab only, its window is
//! what moves); over another window's list that window turns clear and the list opens a slot;
//! let go over a list the tab goes into it, elsewhere its window stays. Nothing is written to the
//! layout before it is let go.

use tao::event_loop::EventLoopWindowTarget;

use super::super::super::chip::Chip;
use super::super::super::deck::Deck;
use super::super::super::strip::Key;
use super::{Cmd, Looping};

/// How far past the window's edge the pointer carries a tab before it comes out (points).
const OUT: f64 = 10.0;

/// Where the pointer is in a window a tab came out into: over its list's first row.
const GRAB: (f64, f64) = (72.0, 54.0);

/// A tab held in a side's list: its card, drawn where the pointer is (chip.rs).
pub(super) struct Held {
    chip: Chip,
}

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

/// A point, in points.
type Point = (f64, f64);

/// A side's message about a carried tab: what, its pane's key, its point in the side's page, and
/// the pointer on the screen when the page says it.
fn said(said: &str) -> Option<(&str, &str, Point, Option<Point>)> {
    let parts: Vec<&str> = said.split(' ').collect();
    let number = |part: &str| part.parse::<f64>().ok();
    match parts.as_slice() {
        [verb @ ("drop-over" | "drop"), key, x, y] => {
            Some((verb, key, (number(x)?, number(y)?), None))
        }
        [verb @ ("drop-over" | "drop"), key, x, y, sx, sy] => Some((
            verb,
            key,
            (number(x)?, number(y)?),
            Some((number(sx)?, number(sy)?)),
        )),
        _ => None,
    }
}

/// `said` as a window's own side reads it: the screen's point is the carrying's only.
pub(super) fn plain(said: &str) -> String {
    let parts: Vec<&str> = said.split(' ').collect();
    match parts.as_slice() {
        [verb @ ("drop-over" | "drop"), key, x, y, _, _] => format!("{verb} {key} {x} {y}"),
        _ => said.to_string(),
    }
}

impl Looping {
    /// A side's message, when it is about a tab carried out of a list (or about to come out):
    /// whether it was, and is done with.
    pub(super) fn tear(&mut self, message: &str, target: &EventLoopWindowTarget<Cmd>) -> bool {
        if let Some(rest) = message.strip_prefix("drag-start ") {
            self.hold(rest);
            return true;
        }
        if message == "drag-end" {
            self.held = None;
            return true;
        }
        if let Some(at) = message.strip_prefix("drag-at ") {
            let mut point = at.split(' ').filter_map(|part| part.parse::<f64>().ok());
            if let (Some(x), Some(y), Some(held)) = (point.next(), point.next(), &self.held) {
                held.chip.follow(x, y);
            }
            return true;
        }
        if let Some(before) = message.strip_prefix("slot ") {
            if let Some(tear) = self.tear.as_mut() {
                tear.before = (before != "end").then(|| before.to_string());
            }
            return true;
        }
        if message == "drop-off" {
            return self.take_back();
        }
        let Some((verb, key, (x, y), on_screen)) = said(message) else {
            return false;
        };
        let Some(key) = Key::parse(key) else {
            return false;
        };
        let from = self
            .tear
            .as_ref()
            .map_or_else(|| self.windows.at(), |tear| tear.from);
        let Some(point) = self.windows.deck(from).and_then(|deck| {
            let top = deck.top();
            // The screen's point as the page says it: a window that moves under the pointer moves
            // the page's points with it, so they and where the window was disagree.
            (on_screen.or_else(|| deck.to_screen(x, y + top))).map(|screen| (screen, (x, y + top)))
        }) else {
            return false;
        };
        match verb {
            "drop" => self.let_go(),
            _ => self.carry(key, point, target),
        }
    }

    /// A tab held in the window in use's side (`GX GY X Y W H [IH]`: where in its card the pointer
    /// holds it, the card's place in the side's page, and the page's height): its card is drawn
    /// where the pointer is from now on, over everything, till it is let go.
    fn hold(&mut self, said: &str) {
        let numbers: Vec<f64> = (said.split(' '))
            .filter_map(|part| part.parse().ok())
            .collect();
        let (gx, gy, x, y, width, height, inner) = match numbers[..] {
            [gx, gy, x, y, width, height] => (gx, gy, x, y, width, height, None),
            [gx, gy, x, y, width, height, inner] => (gx, gy, x, y, width, height, Some(inner)),
            _ => return,
        };
        let at = self.windows.at();
        let Some(side) = self.windows.deck(at).and_then(|deck| deck.side_view()) else {
            return;
        };
        let chip = Chip::lift(side, (x, y, width, height), (gx, gy), inner);
        self.held = Some(Held { chip });
    }

    /// The card seen where the pointer is, or not (a window carries the tab).
    pub(super) fn card(&self, shown: bool) {
        if let Some(held) = &self.held {
            held.chip.show(shown);
        }
    }

    /// The tab is carried to `screen`, a point of the screen (`window`, of its window): out past
    /// the window's edge it comes out, then its window follows, over a list or not.
    fn carry(
        &mut self,
        key: Key,
        (screen, window): ((f64, f64), (f64, f64)),
        target: &EventLoopWindowTarget<Cmd>,
    ) -> bool {
        if self.tear.is_none() {
            let from = self.windows.at();
            let Some((.., wide, tall)) = self.windows.deck(from).and_then(Deck::on_screen) else {
                return false;
            };
            let out = window.0 < -OUT
                || window.1 < -OUT
                || window.0 > wide + OUT
                || window.1 > tall + OUT;
            if !out || !self.come_out(key, from, screen, target) {
                return false;
            }
        }
        let Some(tear) = self.tear.as_ref() else {
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
            if let Some(tear) = self.tear.as_mut() {
                tear.over = over;
                tear.before = None;
            }
        }
        if let Some(deck) = over.and_then(|at| self.windows.deck(at)) {
            let tall = (self.held.as_ref()).map_or(44.0, |held| held.chip.height());
            deck.tell_side(&format!(
                "DS({},{})",
                deck.list_y(screen.1).round(),
                tall.round()
            ));
        }
        true
    }

    /// The tab of `key` comes out of window `from` at `screen`: into a new window that follows
    /// the pointer, or, when it is the only tab the window shows, the window itself follows.
    fn come_out(
        &mut self,
        key: Key,
        from: usize,
        screen: (f64, f64),
        target: &EventLoopWindowTarget<Cmd>,
    ) -> bool {
        // Laid out over the pages a moment ago, the window goes back as it was first.
        self.on(target).command(Cmd::SideClicked("drop-off".into()));
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
            self.tear = Some(Tear {
                keys,
                from,
                moving: from,
                loose: false,
                grab: GRAB,
                over: None,
                before: None,
            });
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
        self.tear = Some(Tear {
            keys,
            from,
            moving,
            loose: true,
            grab: GRAB,
            over: None,
            before: None,
        });
        true
    }
}

#[cfg(test)]
#[path = "host_tear_tests.rs"]
mod tests;
