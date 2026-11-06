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
use super::tab_view::build;
use super::tabs::Tabs;
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
    /// The tabs in the order the person dragged them to; one not in it follows, as it came.
    order: Vec<Key>,
    /// A tab dragged over the pages: whether it would go into the right half (else the left).
    drop: Option<bool>,
    /// The side is open, or slid shut.
    side_open: bool,
    /// The side's width while open, in points: `strip::SIDE` until the person drags its edge.
    side_width: f64,
    /// How much of the side shows now, in points: it slides between its rail and `side_width`.
    side_now: f64,
    /// Which slide is the latest: a step of an earlier one is passed over.
    slide: u32,
    /// The glass under the strip's open list.
    pane: Option<platform::Pane>,
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
        target: &EventLoopWindowTarget<Cmd>,
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            target,
            &window.title,
            &window.viewport,
            window.headless,
            (shared, proxy),
        )?;
        deck.open_web(window, shared, proxy)?;
        Ok(deck)
    }

    /// The window for a terminal session's screen, before the browser opened.
    pub(super) fn for_term(
        target: &EventLoopWindowTarget<Cmd>,
        opening: Opening,
        shared: &Shared,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            target,
            "llx-desk",
            &Viewport::DESKTOP,
            false,
            (shared, proxy),
        )?;
        deck.add_term(opening, true, proxy)?;
        Ok(deck)
    }

    /// The window with nothing in it yet but its strip (when shown), its content `viewport`.
    fn build(
        target: &EventLoopWindowTarget<Cmd>,
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
            false => platform::work_area(target)
                .map(|area| placement::deck(area, crate::lock(&shared.kept).deck)),
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
                kept.side_width.unwrap_or(strip::SIDE),
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
            order: vec![],
            drop: None,
            side_open,
            side_width,
            side_now: if side_open { side_width } else { strip::RAIL },
            slide: 0,
            pane,
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
            super::tabs::see_through_until_drawn(tabs.active(), 1, shared);
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

    /// The two tabs side by side, left and right, when the window is split.
    pub(super) fn pair(&self) -> Option<(Key, Key)> {
        super::split::pair(self.shown_key(), self.split.beside, |key| self.there(key))
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
        // Its tabs are drawn once its page can draw them.
        .with_on_page_load_handler(move |_, _| {
            let _ = loaded.send_event(Cmd::Strip);
        });
    build(builder, native).map_err(|error| error.to_string())
}
