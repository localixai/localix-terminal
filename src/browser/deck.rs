//! One window: the browser's tabs and the terminal sessions' screens are tabs of it, with a
//! strip along its top (strip.rs) and the tabs listed down its left (the side), which the person
//! slides shut and open. The app has several (windows.rs), each its own deck. The browser's tabs
//! are the agent's (tabs.rs): showing a terminal moves them aside, still rendering, and never
//! changes the tab the agent acts on. A shown window opens with the first of either; a headless
//! one is the browser's alone, with no strip. A window the person made stays when nothing is
//! left in it (its start of a new tab shows), and goes when they close it, its tabs with it. A
//! terminal the person closes stays live out of sight, and one whose session ended keeps its last
//! screen, both for the history menu (deck_history.rs) to show again.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tao::event_loop::EventLoopProxy;
use wry::WebView;

use super::frame::{Frame, Look};
use super::host::{Cmd, Shared, Window};
use super::panel::{self, Held, Open, Opening};
use super::platform;
use super::strip::Key;

#[path = "deck_build.rs"]
mod construction;
#[path = "deck_drop.rs"]
mod dropping;
#[path = "deck_render.rs"]
mod render;
#[path = "deck_said.rs"]
mod said;

#[path = "deck_acts.rs"]
mod acts;
#[path = "deck_history.rs"]
mod history;
#[path = "deck_layout.rs"]
mod layout;
#[path = "deck_tear.rs"]
mod tear;
use super::tabs::{self, Tabs};
pub(super) use history::Revisit;

/// How many ended terminals keep their last screen.
const ENDED_KEPT: usize = 8;

/// What fills the window under the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    /// The browser's active tab.
    Web,
    /// A terminal session's screen: its panel's id.
    Term(u32),
}

/// A tab on its way to another window: its page or its panel, with its view as it is.
pub(super) enum Carried {
    Page(tabs::Carried),
    Panel(Open),
}

/// What the strip's menu shows while it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Menu {
    /// The tabs the window has held.
    History,
    /// The Event Center.
    Events,
}

pub(super) struct Deck {
    pub(super) frame: Frame,
    look: Look,
    /// The strip, in a shown window.
    strip: Option<WebView>,
    /// The tabs' column on the window's left: the strip's page in a view of its own. None where
    /// the pages lie in a column (GTK), and the tabs are a row under the strip.
    side: Option<WebView>,
    /// The browser's toolbar over the pages (the strip's page again): back, forward, reload, the
    /// address field, the page's size and comments. It shows over a page only.
    nav: Option<WebView>,
    /// The line between a split window's halves, which the person drags (the strip's page
    /// again): out of sight while the window is whole.
    line: Option<WebView>,
    /// The window split in two, or whole (split.rs).
    pub(super) split: super::split::Split,
    /// A tab dragged over the pages: the half it lands in now (whether the right one), none off
    /// them.
    drop: Option<bool>,
    /// While a tab carried over the pages is laid out where it lands: what was beside the shown
    /// tab before, to go back to when it is carried off them.
    landing: Option<Landing>,
    /// The side is open, or slid shut.
    side_open: bool,
    /// The side's width while open, in points: `strip::SIDE` until the person drags its edge.
    side_width: f64,
    /// How wide the side is now, in points: `side_width` open, its rail shut.
    side_now: f64,
    /// The glass under the strip's open list.
    pane: Option<platform::Pane>,
    /// What each part of the strip (and the page shown, none) was last told, by kind: the same
    /// again is not sent (`Deck::say`).
    said: std::sync::Mutex<said::Said<(Option<Part>, &'static str)>>,
    /// Refreshes held while a command is carried out, drawn once at its end (`Deck::hold`).
    held: said::Held,
    pub(super) web: Option<Tabs>,
    terms: Vec<Open>,
    pub(super) shown: Shown,
    /// The menu open, if one is: it takes the window, the pages stand aside.
    menu: Option<Menu>,
    /// The address field is open: its suggestions take the window as the menu does.
    typing: bool,
    /// What the offer bar under the strip shows now (offer.rs), if anything.
    offer: std::sync::Mutex<Option<super::offer::Bar>>,
    /// The person comments on the pages (probe/feedback.js): a click picks, it does not press.
    commenting: bool,
    /// How many batches of comments were sent: a page drops the marks of earlier ones.
    sent: u32,
    /// The loop's own handle: a tab's picture, drawn later, comes back through it.
    proxy: EventLoopProxy<Cmd>,
}

impl Deck {
    /// Whether the window is on the person's screen (it has its strip), not headless.
    pub(super) fn is_shown(&self) -> bool {
        self.strip.is_some()
    }

    /// Open the browser in the window, and show it.
    pub(super) fn open_web(
        &mut self,
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<(), String> {
        let tabs = Tabs::open(&self.frame.native, window, shared, proxy)?;
        if self.strip.is_none() {
            // A headless window shows with its first page.
            self.frame.show(tabs.active());
        } else {
            // As every new tab: the window shows through until its page drew, never white first.
            tabs::see_through_until_drawn(tabs.active(), 1, shared);
        }
        self.web = Some(tabs);
        self.shown = Shown::Web;
        Ok(())
    }

    /// A terminal's (or a board's) tab added, and shown when it is of the project shown.
    pub(super) fn add_term(
        &mut self,
        opening: Opening,
        shown: bool,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<(), String> {
        let open = panel::open(opening, &self.frame.native, self.look, proxy.clone())?;
        // A session's id used again: the screen kept of the one before is not this one's.
        self.terms.retain(|kept| kept.id != open.id);
        if shown {
            self.shown = Shown::Term(open.id);
        }
        self.terms.push(open);
        Ok(())
    }

    pub(super) fn term(&self, id: u32) -> Option<&Open> {
        self.terms.iter().find(|open| open.id == id)
    }

    /// Terminal `id`'s caller let it go: its last screen is kept, out of the tabs, the oldest
    /// such one let go past a few (its id, then: the tab is gone).
    pub(super) fn end_term(&mut self, id: u32) -> Option<u32> {
        if let Some(open) = self.terms.iter_mut().find(|open| open.id == id) {
            open.held = Held::Ended;
            open.slot.alive.store(false, Ordering::SeqCst);
        }
        let ended = |open: &&Open| open.held == Held::Ended;
        let mut gone = None;
        if self.terms.iter().filter(ended).count() > ENDED_KEPT
            && let Some(at) = self.terms.iter().position(|open| open.held == Held::Ended)
        {
            gone = Some(self.terms.remove(at).id);
        }
        self.settle();
        gone
    }

    /// The person closed terminal `id`'s tab: its program runs on, and its screen is painted
    /// out of sight until it is shown again.
    pub(super) fn hide_term(&mut self, id: u32) {
        self.held(id, Held::Live, Held::Hidden);
        self.settle();
    }

    /// Terminal `id` a tab again, if the person had closed it.
    pub(in super::super) fn unhide_term(&mut self, id: u32) {
        self.held(id, Held::Hidden, Held::Live);
    }

    fn held(&mut self, id: u32, from: Held, to: Held) {
        let found = self.terms.iter_mut().find(|open| open.id == id);
        if let Some(open) = found.filter(|open| open.held == from) {
            open.held = to;
        }
    }

    /// Whether no tab is shown: every one closed. The window stays, and the browser's toolbar
    /// takes the pages' part as the start of a new tab.
    fn blank(&self) -> bool {
        match self.shown {
            Shown::Web => self.web.is_none(),
            Shown::Term(id) => self.term(id).is_none_or(|open| open.held != Held::Live),
        }
    }

    /// The browser is gone from the window (closed by the agent, or its last tab by the person).
    /// The window stays, with no tab left too: then its start of a new tab shows.
    pub(super) fn remove_web(&mut self) {
        self.web = None;
        self.settle();
    }

    /// Every terminal's caller is told the window is gone.
    fn let_go(&self) {
        for open in &self.terms {
            open.slot.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Show `shown`, if it is there.
    pub(super) fn show(&mut self, shown: Shown) {
        let there = match shown {
            Shown::Web => self.web.is_some(),
            Shown::Term(id) => self.term(id).is_some(),
        };
        if there {
            self.shown = shown;
        }
    }

    pub(super) fn titled(&mut self, id: u32, title: String) {
        if let Some(open) = self.terms.iter_mut().find(|open| open.id == id) {
            open.title = title;
        }
    }

    /// The tab shown, as the strip names it: none while an ended screen shows.
    pub(super) fn shown_key(&self) -> Option<Key> {
        match self.shown {
            Shown::Web => self.web.as_ref().map(|web| Key::Web(web.active_id())),
            Shown::Term(id) => (self.term(id))
                .filter(|open| open.held == Held::Live)
                .map(|_| Key::Term(id)),
        }
    }

    /// Whether tab `key` is open in the window.
    fn there(&self, key: Key) -> bool {
        match key {
            Key::Web(id) => (self.web.as_ref()).is_some_and(|web| web.has(id)),
            Key::Term(id) => self.term(id).is_some_and(|open| open.held == Held::Live),
        }
    }

    /// Every pane in this window: its pages, then its terminals and boards.
    pub(super) fn keys(&self) -> Vec<Key> {
        let pages = (self.web.as_ref())
            .map(|web| web.listed().0)
            .unwrap_or_default();
        (pages.into_iter().map(Key::Web))
            .chain(self.terms.iter().map(|open| Key::Term(open.id)))
            .collect()
    }

    /// Whether pane `key` (`w1`, `t2`, `b1`) is in this window, open or kept.
    pub(super) fn has(&self, key: &str) -> bool {
        match Key::parse(key) {
            Some(Key::Web(id)) => (self.web.as_ref()).is_some_and(|web| web.has(id)),
            Some(Key::Term(id)) => self.term(id).is_some(),
            None => false,
        }
    }

    /// Tab `key` leaves for another window, its view as it is (docs/windows.md): another tab
    /// is shown in its place when it was the one shown.
    pub(super) fn take(&mut self, key: Key, shared: &Shared) -> Option<Carried> {
        let shown = self.shown_key() == Some(key);
        let carried = match key {
            Key::Web(id) => {
                let web = self.web.as_mut()?;
                let page = web.take(id, shared)?;
                if web.is_bare() {
                    self.web = None;
                }
                Carried::Page(page)
            }
            Key::Term(id) => {
                let at = (self.terms.iter())
                    .position(|open| open.id == id && open.held == Held::Live)?;
                Carried::Panel(self.terms.remove(at))
            }
        };
        if shown && let Some(next) = self.keys().into_iter().find(|other| *other != key) {
            self.shown = match next {
                Key::Web(_) => Shown::Web,
                Key::Term(id) => Shown::Term(id),
            };
        }
        Some(carried)
    }

    /// A tab come from another window: its view is put in this one as it was, and shown.
    pub(super) fn take_in(&mut self, carried: Carried, shared: &Shared) {
        match carried {
            Carried::Page(page) => {
                let native = &self.frame.native;
                let web = (self.web).get_or_insert_with(|| Tabs::empty(native, page.window()));
                web.take_in(page, native, shared);
                self.shown = Shown::Web;
            }
            Carried::Panel(open) => {
                platform::reparent(&open.webview, &self.frame.native);
                self.shown = Shown::Term(open.id);
                self.terms.retain(|kept| kept.id != open.id);
                self.terms.push(open);
            }
        }
    }

    /// The two tabs side by side, left and right, when the window is split.
    pub(super) fn pair(&self) -> Option<(Key, Key)> {
        self.pair_with(self.split.beside)
    }

    /// The two halves' tabs, left and right, were `beside` the tab beside the shown one.
    pub(super) fn pair_with(&self, beside: Option<super::split::Beside>) -> Option<(Key, Key)> {
        super::split::pair(self.shown_key(), beside, |key| self.there(key))
    }

    /// Split the window, tab `key` beside the one shown, on the right; or make it whole (none).
    pub(super) fn split_with(&mut self, key: Option<Key>) {
        self.split.beside = key.map(|key| super::split::Beside { key, right: true });
        self.split.grabbed = None;
    }

    /// Whether `window` (an NSWindow's address) is this window.
    #[cfg(target_os = "macos")]
    pub(super) fn is_window(&self, window: usize) -> bool {
        use tao::platform::macos::WindowExtMacOS;
        self.frame.native.ns_window().addr() == window
    }

    /// Elsewhere no press is told of.
    #[cfg(not(target_os = "macos"))]
    #[expect(
        clippy::unused_self,
        reason = "the deck's question on macOS, where it reads the window"
    )]
    pub(super) fn is_window(&self, _window: usize) -> bool {
        false
    }

    /// What is shown is still a tab: else the newest terminal's, else the browser. (An ended
    /// screen the history menu showed is let go of here too.)
    fn settle(&mut self) {
        let gone = match self.shown {
            Shown::Web => self.web.is_none(),
            Shown::Term(id) => self.term(id).is_none_or(|open| open.held != Held::Live),
        };
        if gone {
            let live = self.terms.iter().rev().find(|open| open.held == Held::Live);
            self.shown = match live {
                Some(open) => Shown::Term(open.id),
                None => Shown::Web,
            };
        }
    }
}

impl Drop for Deck {
    /// Every terminal's caller is told the window is gone, a hidden one's too.
    fn drop(&mut self) {
        self.let_go();
    }
}

/// Which part of the window the strip's page draws.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Part {
    Strip,
    Side,
    Nav,
    /// The line between a split window's halves.
    Line,
}

/// How many times a part of the strip loaded its page: one that loads again has lost what it was
/// told, so the next refresh tells it all again (`Deck::say`).
pub(super) static LOADS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// What was beside the shown tab before a tab carried over the pages was laid out where it
/// lands.
#[derive(Debug, Clone, Copy)]
pub(super) struct Landing {
    pub(super) was: Option<super::split::Beside>,
}

/// Where a point of the window is for a tab carried there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Over {
    /// Off the pages.
    Off,
    /// The middle of a whole window's pages: where the tab lands stays as it is.
    Middle,
    /// A half of the pages, the right one or the left.
    Half(bool),
}
