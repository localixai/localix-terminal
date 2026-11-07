//! The one window: the browser's tabs and the terminal sessions' screens are tabs of it, with a
//! strip along its top (strip.rs) and the tabs listed down its left (the side), which the person
//! slides shut and open. The browser's tabs
//! are the agent's (tabs.rs): showing a terminal moves them aside, still rendering, and never
//! changes the tab the agent acts on. A shown window opens with the first of either; a headless
//! one is the browser's alone, with no strip. It closes when nothing is left in it. A terminal
//! the person closes stays live out of sight, and one whose session ended keeps its last screen,
//! both for the history menu (deck_history.rs) to show again.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

use super::frame::{self, Frame, Look};
use super::host::{Cmd, Shared, Window};
use super::panel::{self, Held, Open, Opening};
use super::strip::{self, HEIGHT, Key};
use super::{placement, platform};

#[path = "deck_acts.rs"]
mod acts;
#[path = "deck_history.rs"]
mod history;
#[path = "deck_layout.rs"]
mod layout;
#[path = "deck_tear.rs"]
mod tear;
use super::tab_view::build;
use super::tabs::{self, Tabs};
use super::viewport::Viewport;
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
    /// What each part of the strip was last told, by kind (a hash of the script), and the count
    /// of page loads it was told since: the same again is not sent (`Deck::say`).
    pub(super) said: std::sync::Mutex<Said>,
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

    /// The window for the browser, on its first tab.
    pub(super) fn for_web(
        (target, main): (&EventLoopWindowTarget<Cmd>, bool),
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            (target, main),
            &window.title,
            &window.viewport,
            window.headless,
            (shared, proxy),
        )?;
        deck.open_web(window, shared, proxy)?;
        Ok(deck)
    }

    /// A window with nothing in it yet, not the main one: a tab carried out of a list comes into
    /// it (docs/windows.md).
    pub(super) fn empty(
        target: &EventLoopWindowTarget<Cmd>,
        shared: &Shared,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        Self::build(
            (target, false),
            "llx-desk",
            &Viewport::DESKTOP,
            false,
            (shared, proxy),
        )
    }

    /// The window for a terminal session's screen, before the browser opened.
    pub(super) fn for_term(
        (target, main): (&EventLoopWindowTarget<Cmd>, bool),
        opening: Opening,
        shared: &Shared,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            (target, main),
            "llx-desk",
            &Viewport::DESKTOP,
            false,
            (shared, proxy),
        )?;
        deck.add_term(opening, true, proxy)?;
        Ok(deck)
    }

    /// The window with nothing in it yet but its strip (when shown), its content `viewport`.
    /// Not the `main` window, it opens cascaded from where the main one is kept.
    fn build(
        (target, main): (&EventLoopWindowTarget<Cmd>, bool),
        title: &str,
        viewport: &Viewport,
        headless: bool,
        (shared, proxy): (&Shared, &EventLoopProxy<Cmd>),
    ) -> Result<Self, String> {
        let look = frame::look(headless, frame::transparency());
        let top = if headless { 0.0 } else { HEIGHT };
        // A shown window opens on the right of the person's screen, or where they left it; a
        // hidden one keeps the page's own size.
        let placed = match headless {
            true => None,
            false => platform::work_area(target).map(|area| {
                let deck = placement::deck(area, crate::lock(&shared.kept).deck);
                if main {
                    deck
                } else {
                    placement::cascade(area, deck)
                }
            }),
        };
        let (size, at) = match placed {
            Some(at) => (
                LogicalSize::new(at.width, at.height),
                Some(LogicalPosition::new(at.x, at.y)),
            ),
            None => (
                LogicalSize::new(viewport.width, viewport.height + top),
                None,
            ),
        };
        // Coated in the person's theme: the titlebar and the strip are not the bare material's
        // grey. The browser's pages paint their own ground over it, opaque.
        let mut frame = Frame::build(target, (title, size, at), look)?;
        let strip = match headless {
            true => None,
            false => {
                let strip = strip_view(&frame.native, look, proxy.clone())?;
                platform::collapse_button(&frame.native, proxy);
                // Shows the window too, without making it the key window.
                frame.show(&strip);
                Some(strip)
            }
        };
        // The tabs' column, where the pages lie side by side with it (not in GTK's column).
        let side = match (&strip, cfg!(target_os = "linux")) {
            (Some(_), false) => Some(side_view(&frame.native, look, proxy.clone())?),
            _ => None,
        };
        let nav = match &strip {
            Some(_) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Nav)?),
            None => None,
        };
        // Split only where the pages lie side by side with the side (not in GTK's column).
        let line = match &side {
            Some(_) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Line)?),
            None => None,
        };
        let (side_open, side_width) = {
            let kept = crate::lock(&shared.kept);
            (
                kept.side.unwrap_or(true),
                (kept.side_width).map_or(strip::SIDE, |width| {
                    width.clamp(strip::SIDE_MIN, strip::SIDE_MAX)
                }),
            )
        };
        let pane = strip.as_ref().and_then(platform::Pane::under);
        Ok(Self {
            frame,
            look,
            strip,
            side,
            nav,
            line,
            split: super::split::Split::default(),
            drop: None,
            landing: None,
            side_open,
            side_width,
            side_now: if side_open { side_width } else { strip::RAIL },
            pane,
            said: std::sync::Mutex::default(),
            web: None,
            terms: vec![],
            shown: Shown::Web,
            menu: None,
            typing: false,
            offer: std::sync::Mutex::default(),
            commenting: false,
            sent: 0,
            proxy: proxy.clone(),
        })
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

    /// Add a terminal session's screen as a tab, and show it.
    /// A terminal's (or a board's) tab added; `shown` when it is of the project shown.
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
    /// such one let go past a few. Whether anything is left in the window.
    pub(super) fn end_term(&mut self, id: u32) {
        if let Some(open) = self.terms.iter_mut().find(|open| open.id == id) {
            open.held = Held::Ended;
            open.slot.alive.store(false, Ordering::SeqCst);
        }
        let ended = |open: &&Open| open.held == Held::Ended;
        if self.terms.iter().filter(ended).count() > ENDED_KEPT
            && let Some(at) = self.terms.iter().position(|open| open.held == Held::Ended)
        {
            self.terms.remove(at);
        }
        self.settle();
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

    /// What is shown is still a tab: else the newest terminal's, else the browser. (An ended
    /// screen the history menu showed is let go of here too.)
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
            Key::Web(id) => (self.web.as_ref()).is_some_and(|web| web.listed().0.contains(&id)),
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
            Some(Key::Web(id)) => {
                (self.web.as_ref()).is_some_and(|web| web.listed().0.contains(&id))
            }
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
    pub(super) fn is_window(&self, _window: usize) -> bool {
        false
    }

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

/// The strip's page, which says what the person clicked and nothing else.
fn strip_view(
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<WebView, String> {
    strip_page(native, look, proxy, Part::Strip)
}

/// The side: the strip's page again, listing the tabs; what the person clicks there is said as
/// the strip says it, from the side (its points are the side's).
fn side_view(
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<WebView, String> {
    strip_page(native, look, proxy, Part::Side)
}

/// Which part of the window the strip's page draws.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Strip,
    Side,
    Nav,
    /// The line between a split window's halves.
    Line,
}

/// What the parts of the strip were last told: the page loads counted then, and each part's
/// script of each kind, hashed.
pub(super) type Said = (u32, std::collections::HashMap<(usize, &'static str), u64>);

/// How many times a part of the strip loaded its page: one that loads again has lost what it was
/// told, so the next refresh tells it all again (`Deck::say`).
pub(super) static LOADS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn strip_page(
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
    part: Part,
) -> Result<WebView, String> {
    // Every part draws the agents' marks (the side's tabs, the bar's crumbs and offer).
    let page = frame::dress(strip::PAGE, look)
        .replacen("/*MARKS*/{}", &super::agents::marks(), 1)
        .replacen("/*AGENTS*/[]", &super::agents::installed(), 1);
    let page = match part {
        Part::Strip => page,
        Part::Side => page.replacen("<html", "<html data-part=\"side\"", 1),
        Part::Nav => page.replacen("<html", "<html data-part=\"nav\"", 1),
        Part::Line => page.replacen("<html", "<html data-part=\"line\"", 1),
    };
    let first = AtomicBool::new(true);
    let loaded = proxy.clone();
    let builder = WebViewBuilder::new()
        // The first click in a window that is not the key one reaches the page: the person
        // never clicks twice, once to wake the window and once to do what they meant.
        .with_accept_first_mouse(true)
        .with_html(page)
        // The person's fonts, before the page's own scripts (settings.rs).
        .with_initialization_script(super::settings::look_script(&super::settings::now()))
        .with_transparent(look == Look::Glass)
        .with_incognito(true)
        .with_focused(false)
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_ipc_handler(move |request| {
            let said = request.body().clone();
            let _ = proxy.send_event(match part {
                Part::Strip | Part::Line => Cmd::Clicked(said),
                Part::Side => Cmd::SideClicked(said),
                Part::Nav => Cmd::NavClicked(said),
            });
        })
        // Its tabs are drawn once its page can draw them, all it was told told again.
        .with_on_page_load_handler(move |_, _| {
            LOADS.fetch_add(1, Ordering::SeqCst);
            let _ = loaded.send_event(Cmd::Strip);
        });
    build(builder, native).map_err(|error| error.to_string())
}

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
