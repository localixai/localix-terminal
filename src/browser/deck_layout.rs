//! The window's layout: the strip along the top, the tabs' column (the side) down the left under
//! it, what is shown beside that, the rest out of sight, and the strip and the side told what the
//! window holds. While the history menu is open the strip takes the whole window.

use std::sync::mpsc::SyncSender;

use tao::dpi::LogicalSize;

use super::super::capture::{Shot, capture};
use super::super::host::Cmd;
use super::super::tabs::Tabs;

use super::super::host::Shared;

use super::super::strip::{self, BAR, HEIGHT, Item, Key, NAV, SIDE_MAX, SIDE_MIN};
use super::super::tabs::{pin, place};
use super::{Deck, Menu, Shown};

impl Deck {
    /// What the mini shows: the tab shown, and which of the browser's tabs it is.
    pub(in super::super) fn signature(&self) -> (Shown, u32) {
        (
            self.shown,
            self.web.as_ref().map_or(0, |web| web.listed().1),
        )
    }

    /// The tab shown, as the mini names it: its key and its project.
    pub(in super::super) fn shown_tab(&self, shared: &Shared) -> Option<(String, String)> {
        let listed = self.listing(shared);
        let tab = listed.into_iter().find(|tab| tab.shown)?;
        Some((tab.id, tab.group))
    }

    /// A picture of the tab shown, for the mini; the reply says why when there is none.
    pub(in super::super) fn capture_shown(&self, reply: SyncSender<Result<Shot, String>>) {
        let view = match self.shown {
            Shown::Web => self.web.as_ref().map(Tabs::active),
            Shown::Term(id) => self.term(id).map(|open| &open.webview),
        };
        match view {
            Some(view) => capture(view, None, reply),
            None => {
                let _ = reply.send(Err("no tab is shown".into()));
            }
        }
    }

    /// How tall the strip is: its bar, the tabs' row where there is no side, and an offer.
    pub(in super::super) fn top(&self) -> f64 {
        if self.strip.is_none() {
            return 0.0;
        }
        let own = if self.side.is_some() { BAR } else { HEIGHT };
        let offering = crate::lock(&self.offer).is_some();
        let offer = if offering {
            super::super::offer::ROW
        } else {
            0.0
        };
        own + offer
    }

    /// Whether the browser's toolbar shows: a page is the tab shown.
    fn browsing(&self) -> bool {
        self.nav.is_some() && self.shown == Shown::Web && self.web.is_some()
    }

    /// Where the browser's toolbar lies in the window: over the pages, beside the side.
    pub(in super::super) fn nav_at(&self) -> (f64, f64) {
        (self.left(), self.top())
    }

    /// What the offer bar shows now.
    pub(in super::super) fn offered(&self) -> Option<super::super::offer::Bar> {
        crate::lock(&self.offer).clone()
    }

    /// How much of the window's left the side takes now.
    fn left(&self) -> f64 {
        if self.side.is_some() {
            self.side_now
        } else {
            0.0
        }
    }

    /// The shown tab's part of the window, under the strip (and a page's toolbar) and beside the
    /// side: its size, and how far from the window's left and top it starts.
    pub(super) fn content(&self) -> (LogicalSize<f64>, f64, f64) {
        self.area(self.browsing())
    }

    /// The tabs' part of the window: a page's under its toolbar (`nav`), a terminal's under the
    /// strip.
    fn area(&self, nav: bool) -> (LogicalSize<f64>, f64, f64) {
        let native = &self.frame.native;
        let size = native.inner_size().to_logical::<f64>(native.scale_factor());
        let left = self.left();
        let top = self.top() + if nav { NAV } else { 0.0 };
        (
            LogicalSize::new((size.width - left).max(0.0), (size.height - top).max(0.0)),
            left,
            top,
        )
    }

    /// The strip along the top, what is shown under it, and the rest beside, out of sight.
    pub(in super::super) fn layout(&self) {
        let (size, left, top) = self.area(false);
        let width = size.width + left;
        // The open menu has the whole window, the address field's suggestions the pages' part:
        // the pages stand aside, still rendering.
        let over = self.menu.is_some() || self.typing;
        let mut raised = false;
        if let Some(strip) = &self.strip {
            let height = if self.menu.is_some() {
                top + size.height
            } else {
                top
            };
            pin(strip, LogicalSize::new(width, height));
            raised = super::super::platform::raise(strip);
        }
        if let Some(nav) = &self.nav {
            let browsing = self.browsing();
            let x = if browsing { left } else { -(width + 100.0) };
            let tall = if self.typing { size.height } else { NAV };
            let _ = nav.set_bounds(super::super::frame::fill(
                LogicalSize::new(size.width, tall),
                x,
                top,
            ));
            if self.typing {
                raised = super::super::platform::raise(nav) || raised;
            }
        }
        // The side slides in from the left, its width kept: what of it shows is `left`. An open
        // menu leaves it in place: the strip, clear but for the menu, lies over it then.
        if let Some(side) = &self.side {
            let shows = left > 0.5;
            let wide = self.side_width;
            let x = if shows { left - wide } else { -(wide + 100.0) };
            let column = LogicalSize::new(wide, size.height);
            let _ = side.set_bounds(super::super::frame::fill(column, x, top));
            super::super::platform::follow(side, super::super::platform::Follow::Side);
        }
        // Where the strip lies over the pages, they stay under an open menu.
        let over = over && !raised;
        if let Some(web) = &self.web {
            let (size, left, top) = self.area(self.nav.is_some());
            web.layout(size, left, top, self.shown == Shown::Web && !over);
        }
        for open in &self.terms {
            let here = self.shown == Shown::Term(open.id) && !over;
            let x = if here { left } else { -(width + 100.0) };
            place(&open.webview, x, top, size, here);
        }
    }

    /// The side slid open or shut, as the person asked: in a few steps, each a `Cmd::Slide` the
    /// loop lays out; a newer slide passes over the steps of one under way.
    pub(in super::super) fn slide_side(&mut self, shared: &Shared) {
        if self.side.is_none() {
            return;
        }
        self.side_open = !self.side_open;
        crate::lock(&shared.kept).side = Some(self.side_open);
        self.slide = self.slide.wrapping_add(1);
        let to = if self.side_open { self.side_width } else { 0.0 };
        let (from, slide) = (self.side_now, self.slide);
        let proxy = self.proxy.clone();
        let started = std::thread::Builder::new()
            .name("desk-slide".into())
            .spawn(move || {
                /// Steps of the slide, a frame apart: about a sixth of a second in all.
                const STEPS: u32 = 10;
                for step in 1..=STEPS {
                    #[expect(
                        clippy::disallowed_methods,
                        reason = "a slide is drawn a frame at a time: the wait is the frame"
                    )]
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    // Eased out: fast first, settling into place.
                    let done = 1.0 - (1.0 - f64::from(step) / f64::from(STEPS)).powi(3);
                    if proxy
                        .send_event(Cmd::Slide(slide, from + (to - from) * done))
                        .is_err()
                    {
                        return;
                    }
                }
            });
        if started.is_err() {
            self.side_now = to;
            self.layout();
        }
    }

    /// The person drags the side's edge to `width` points: the side and the pages beside it
    /// follow at once, held between `SIDE_MIN` and `SIDE_MAX` (and half the window); let go
    /// (`done`), the width is kept for the next window.
    pub(in super::super) fn size_side(&mut self, width: f64, done: bool, shared: &Shared) {
        if self.side.is_none() || !self.side_open {
            return;
        }
        let native = &self.frame.native;
        let window = native
            .inner_size()
            .to_logical::<f64>(native.scale_factor())
            .width;
        let widest = SIDE_MAX.min(window / 2.0).max(SIDE_MIN);
        let width = width.clamp(SIDE_MIN, widest).round();
        // A slide under way gives way to the hand.
        self.slide = self.slide.wrapping_add(1);
        let moved = (width - self.side_width).abs() >= 0.5 || (width - self.side_now).abs() >= 0.5;
        self.side_width = width;
        self.side_now = width;
        if moved {
            self.layout();
        }
        if done {
            crate::lock(&shared.kept).side_width = Some(width);
            self.refresh(shared);
        }
    }

    /// A step of slide `slide`: `now` of the side shows. The last one tells the strip.
    pub(in super::super) fn slid(&mut self, slide: u32, now: f64, shared: &Shared) {
        if slide != self.slide {
            return;
        }
        self.side_now = now;
        self.layout();
        let to = if self.side_open { self.side_width } else { 0.0 };
        if (now - to).abs() < 0.5 {
            self.refresh(shared);
        }
    }

    /// Lay the window out again and tell the strip what it holds now.
    pub(in super::super) fn refresh(&self, shared: &Shared) {
        let listed = self.listing(shared);
        super::super::platform::page_shown(self.shown == Shown::Web && self.web.is_some());
        let shown_key = (listed.iter().find(|tab| tab.shown)).and_then(|tab| Key::parse(&tab.id));
        let bar = shown_key.and_then(|key| shared.offers.bar(key, &self.proxy));
        crate::lock(&self.offer).clone_from(&bar);
        self.layout();
        // Again on every refresh: a tab switched to, or a page loaded, comments as the strip says.
        if let Some(web) = &self.web {
            let script = strip::comment(self.commenting, self.sent);
            let _ = web.active().evaluate_script(&script);
        }
        let Some(strip) = &self.strip else {
            return;
        };
        let _ = strip.evaluate_script(&super::super::offer::script(bar.as_ref()));
        let shown = listed
            .iter()
            .find(|tab| tab.shown)
            .map(|tab| tab.id.clone());
        let items: Vec<Item> = listed
            .into_iter()
            .filter_map(|tab| {
                Some(Item {
                    key: Key::parse(&tab.id)?,
                    title: tab.title,
                    detail: tab.detail,
                    agent: shared.offers.agent(&tab.id),
                    group: tab.group,
                })
            })
            .collect();
        let shown = shown.and_then(|id| Key::parse(&id)).unwrap_or(Key::Web(0));
        let side = self.side.as_ref().map(|_| self.side_open);
        let places = crate::lock(&shared.places).clone();
        let scope = crate::lock(&shared.scope).clone();
        let project = self.shown_project(shared);
        let projects = strip::Projects {
            shown: &project,
            places: (places.iter())
                .map(|place| (place, scope.marked(&place.key)))
                .collect(),
        };
        let drawn = strip::script(&items, shown, &self.visited(shared), side, &projects);
        for view in [Some(strip), self.side.as_ref(), self.nav.as_ref()]
            .into_iter()
            .flatten()
        {
            let _ = view.evaluate_script(&drawn);
        }
        let rows = (self.menu == Some(Menu::History)).then(|| self.rows(shared));
        if self.menu != Some(Menu::Events) {
            let _ = strip.evaluate_script(&strip::menu(rows.as_deref()));
        }
        let events = crate::lock(&shared.center).clone();
        let center = strip::center(&events, self.menu == Some(Menu::Events));
        let _ = strip.evaluate_script(&center);
        let framed = (self.web.as_ref())
            .filter(|_| self.shown == Shown::Web)
            .and_then(|web| web.frame(None));
        // The page's size, comments and whose turn it is: the browser's toolbar's.
        let Some(nav) = &self.nav else {
            return;
        };
        let (size, _, top) = self.content();
        let scale = super::super::sizes::place(size, top, framed).scale;
        let _ = nav.evaluate_script(&super::super::sizes::script(framed, scale));
        let held = shared.comments.count();
        let _ = nav.evaluate_script(&strip::commenting(self.commenting, held));
        let (acting, waiting) = shared.turn.now();
        let turn = strip::Turn::of(acting, self.commenting, waiting);
        let _ = nav.evaluate_script(&strip::turn(turn));
    }
}
