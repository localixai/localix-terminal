//! The window's layout: the strip along the top, the tabs' column (the side) down the left under
//! it, what is shown beside that, the rest out of sight, and the strip and the side told what the
//! window holds. While the history menu is open the strip takes the whole window.

use std::sync::mpsc::SyncSender;

use tao::dpi::LogicalSize;

use super::super::capture::{Shot, capture};
use super::super::host::Cmd;
use super::super::tabs::Tabs;

use super::super::host::Shared;

use super::super::strip::{self, BAR, HEIGHT, Item, Key, NAV, RAIL, SIDE_MAX, SIDE_MIN};
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
    /// side, its half of a split one: its size, and how far from the window's left and top it
    /// starts.
    pub(super) fn content(&self) -> (LogicalSize<f64>, f64, f64) {
        let (size, left, top) = self.area(self.browsing());
        match self.halves(size.width, left) {
            Some(halves) => {
                let now = self.shown_key();
                let (_, x, width) = halves
                    .into_iter()
                    .find(|half| Some(half.0) == now)
                    .unwrap_or(halves[0]);
                (LogicalSize::new(width, size.height), x, top)
            }
            None => (size, left, top),
        }
    }

    /// A split window's halves across `width` points from `left`: each one's tab, left and
    /// width.
    fn halves(&self, width: f64, left: f64) -> Option<[(Key, f64, f64); 2]> {
        let (first, second) = self.pair()?;
        let ((x, w), (x2, w2)) = super::super::split::halves(width, left, self.split.share);
        Some([(first, x, w), (second, x2, w2)])
    }

    /// The tab in the half at `x`, `y` of the window (points from its top left), when the window
    /// is split and the press is on a page.
    pub(in super::super) fn half_at(&self, x: f64, y: f64) -> Option<Key> {
        let (size, left, top) = self.area(false);
        if y < top {
            return None;
        }
        let halves = self.halves(size.width, left)?;
        (halves.into_iter())
            .find(|&(_, from, width)| x >= from && x < from + width)
            .map(|half| half.0)
    }

    /// Which half of the pages `x`, `y` of the window is over (whether the right one): a split
    /// window's own, a whole one's even halves. None off the pages.
    pub(in super::super) fn drop_target(&self, x: f64, y: f64) -> Option<bool> {
        let (size, left, top) = self.area(false);
        if x < left || y < top {
            return None;
        }
        let right = match self.halves(size.width, left) {
            Some([_, second]) => second.1,
            None => super::super::split::halves(size.width, left, 0.5).1.0,
        };
        Some(x >= right)
    }

    /// A tab dragged over the pages at `at` (a point of the window), or not (none): the half it
    /// would go into shows.
    pub(in super::super) fn drop_over(&mut self, at: Option<(f64, f64)>) {
        let drop = at.and_then(|(x, y)| self.drop_target(x, y));
        if drop == self.drop {
            return;
        }
        self.drop = drop;
        if let Some(line) = &self.line {
            let flag = if drop.is_some() { "true" } else { "false" };
            let _ = line.evaluate_script(&format!(
                "document.documentElement.toggleAttribute('data-drop', {flag})"
            ));
        }
        self.layout();
    }

    /// The line between the halves dragged `moved` points since the person took hold of it; let
    /// go of when `done`.
    pub(in super::super) fn drag_split(&mut self, moved: i32, done: bool) {
        let (size, ..) = self.area(false);
        let room = (size.width - super::super::split::GAP).max(1.0);
        let from = *self.split.grabbed.get_or_insert(self.split.share);
        self.split.share = super::super::split::held(from + f64::from(moved) / room);
        if done {
            self.split.grabbed = None;
        }
        self.layout();
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
        // Split, the window shows two tabs, each in its half; the one in use has the toolbar.
        let halves = if over {
            None
        } else {
            self.halves(size.width, left)
        };
        let in_use = self.shown_key();
        let (used_x, used_width) = (halves.iter().flatten())
            .find(|half| Some(half.0) == in_use)
            .map_or((left, size.width), |half| (half.1, half.2));
        if let Some(nav) = &self.nav {
            let blank = self.nav.is_some() && self.blank();
            let x = if self.browsing() || blank {
                used_x
            } else {
                -(width + 100.0)
            };
            let tall = if self.typing || blank {
                size.height
            } else {
                NAV
            };
            let wide = if blank { size.width } else { used_width };
            let _ = nav.set_bounds(super::super::frame::fill(
                LogicalSize::new(wide, tall),
                x,
                top,
            ));
            if self.typing {
                raised = super::super::platform::raise(nav) || raised;
            }
        }
        // The side narrows to its rail and widens again, its page laid out at each width: `left`
        // of it shows. An open menu leaves it in place: the strip, clear but for the menu, lies
        // over it then.
        if let Some(side) = &self.side {
            let column = LogicalSize::new(left, size.height);
            let _ = side.set_bounds(super::super::frame::fill(column, 0.0, top));
            super::super::platform::follow(side, super::super::platform::Follow::Side);
        }
        // Where the strip lies over the pages, they stay under an open menu.
        let over = over && !raised;
        if let Some(web) = &self.web {
            let toolbar = if self.nav.is_some() { NAV } else { 0.0 };
            let pages: Vec<super::super::tabs::Pane> = match halves {
                Some(halves) => (halves.into_iter())
                    .filter_map(|(key, x, w)| match key {
                        Key::Web(id) => {
                            let above = if Some(key) == in_use { toolbar } else { 0.0 };
                            let tall = (size.height - above).max(0.0);
                            Some((id, LogicalSize::new(w, tall), x, top + above))
                        }
                        Key::Term(_) => None,
                    })
                    .collect(),
                None if self.shown == Shown::Web && !over => {
                    let (size, left, top) = self.area(self.nav.is_some());
                    vec![(web.active_id(), size, left, top)]
                }
                None => vec![],
            };
            web.layout(&pages);
        }
        for open in &self.terms {
            let spot = match halves {
                Some(halves) => (halves.into_iter())
                    .find(|half| half.0 == Key::Term(open.id))
                    .map(|half| (half.1, half.2)),
                None => (self.shown == Shown::Term(open.id) && !over).then_some((left, size.width)),
            };
            match spot {
                Some((x, w)) => place(
                    &open.webview,
                    x,
                    top,
                    LogicalSize::new(w, size.height),
                    true,
                ),
                None => place(&open.webview, -(width + 100.0), top, size, false),
            }
        }
        self.place_line(halves, (size, left, top), width);
    }

    /// The line's view: over the half a tab dragged out over the pages would go into, else on the
    /// line between a split window's halves, else out of sight.
    fn place_line(
        &self,
        halves: Option<[(Key, f64, f64); 2]>,
        (size, left, top): (LogicalSize<f64>, f64, f64),
        width: f64,
    ) {
        // A tab dragged over the pages: the line's view lies over the half it would go into.
        if let (Some(line), Some(right)) = (&self.line, self.drop) {
            let (first, second) = match self.halves(size.width, left) {
                Some([first, second]) => ((first.1, first.2), (second.1, second.2)),
                None => super::super::split::halves(size.width, left, 0.5),
            };
            let (x, w) = if right { second } else { first };
            let _ = line.set_bounds(super::super::frame::fill(
                LogicalSize::new(w, size.height),
                x,
                top,
            ));
            super::super::platform::raise(line);
        } else if let Some(line) = &self.line {
            let at = halves.map(|[_, second]| second.1 - super::super::split::GAP);
            let x = at.map_or(-(width + 100.0), |at| {
                at + super::super::split::GAP / 2.0 - super::super::split::LINE / 2.0
            });
            let column = LogicalSize::new(super::super::split::LINE, size.height);
            let _ = line.set_bounds(super::super::frame::fill(column, x, top));
            if at.is_some() {
                super::super::platform::raise(line);
            }
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
        let to = if self.side_open {
            self.side_width
        } else {
            RAIL
        };
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
        let to = if self.side_open {
            self.side_width
        } else {
            RAIL
        };
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
        // The split's two tabs, left and right: the side lists them as one, the bar's button lit
        // while the window shows them; another tab shown, the project's split still listed so.
        let on = self.pair().map(|(left, right)| [left.name(), right.name()]);
        let kept = crate::lock(&shared.splits)
            .get(&project)
            .map(super::super::split::sides);
        let split = format!(
            "SP({}, {})",
            serde_json::json!(on.clone().or(kept)),
            on.is_some()
        );
        for view in [Some(strip), self.side.as_ref(), self.nav.as_ref()]
            .into_iter()
            .flatten()
        {
            let _ = view.evaluate_script(&split);
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
