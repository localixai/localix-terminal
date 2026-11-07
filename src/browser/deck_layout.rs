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
use super::{Deck, Landing, Menu, Over, Shown};

/// The middle of a whole window, as a share of its width either side of the centre, where a
/// tab dragged over the pages goes into no half.
const DEAD_MIDDLE: f64 = 0.18;

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

    /// The keys go to the tab shown, after the person switched to it themselves: never while
    /// the window is not the key window (an agent's switch, or another app in front), while the
    /// person types in the strip, or while a menu is open.
    pub(in super::super) fn focus_shown(&self) {
        if self.typing || self.menu.is_some() || !self.frame.is_key() {
            return;
        }
        let view = match self.shown {
            Shown::Web => self.web.as_ref().map(Tabs::active),
            Shown::Term(id) => self.term(id).map(|open| &open.webview),
        };
        if let Some(view) = view {
            let _ = view.focus();
        }
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

    /// The side at its width, its rail when shut, `tall` points high from `top`.
    fn place_side(&self, tall: f64, top: f64) {
        let Some(side) = &self.side else {
            return;
        };
        let column = LogicalSize::new(self.side_now, tall);
        let _ = side.set_bounds(super::super::frame::fill(column, 0.0, top));
        super::super::platform::follow(side, super::super::platform::Follow::Side);
    }

    /// How much of the window's left the side keeps now: the tabs lie beside it.
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
        // A share kept from a wider window, or the side opening, never leaves a half too narrow.
        let room = (width - super::super::split::GAP).max(1.0);
        let share = super::super::split::held(self.split.share, room);
        let ((x, w), (x2, w2)) = super::super::split::halves(width, left, share);
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
        // A press on the line takes hold of it: the half beside it stays as it is.
        let seam = halves[1].1 - super::super::split::GAP / 2.0;
        if (x - seam).abs() <= super::super::split::LINE / 2.0 {
            return None;
        }
        (halves.into_iter())
            .find(|&(_, from, width)| x >= from && x < from + width)
            .map(|half| half.0)
    }

    /// Where `x`, `y` of the window is for a tab carried there: a split window's own half, a
    /// whole one's even halves (its middle none), or off the pages.
    pub(in super::super) fn over(&self, x: f64, y: f64) -> Over {
        let (size, left, top) = self.area(false);
        // Off the pages, past the window's edges too, it goes nowhere.
        if x < left || y < top || x > left + size.width || y > top + size.height {
            return Over::Off;
        }
        let Some([_, second]) = self.halves(size.width, left) else {
            // A whole window splits only for a tab taken well to one side: its middle is no half.
            let off = (x - (left + size.width / 2.0)) / size.width.max(1.0);
            return if off.abs() < DEAD_MIDDLE {
                Over::Middle
            } else {
                Over::Half(off > 0.0)
            };
        };
        Over::Half(x >= second.1)
    }

    /// Tab `key` carried over the pages at `at` (a point of the window), or off them (none):
    /// the window is laid out as it will be where the tab lands, the halves as they will be, and
    /// back as it was off the pages. Over a whole window's middle it stays as it is.
    pub(in super::super) fn drop_over(&mut self, key: Key, at: Option<(f64, f64)>) {
        let over = at.map_or(Over::Off, |(x, y)| self.over(x, y));
        let side = match over {
            Over::Middle => return,
            Over::Off => None,
            Over::Half(right) => Some(right),
        };
        if side == self.drop {
            return;
        }
        self.drop = side;
        // Where it lands is worked out from the window as it was before the tab came over it.
        let was = self
            .landing
            .map_or(self.split.beside, |landing| landing.was);
        let landed = side.and_then(|right| {
            super::super::split::landed(was, self.shown_key(), self.pair_with(was), key, right)
        });
        match landed {
            Some(beside) => {
                self.landing.get_or_insert(Landing { was });
                self.split.beside = Some(beside);
                self.split.grabbed = None;
            }
            None => {
                if let Some(landing) = self.landing.take() {
                    self.split.beside = landing.was;
                }
            }
        }
        self.layout();
    }

    /// The tab carried over the pages taken off them: the window as it was before.
    pub(in super::super) fn drop_off(&mut self) {
        let was = self.landing.take();
        self.drop = None;
        if let Some(landing) = was {
            self.split.beside = landing.was;
            self.layout();
        }
    }

    /// The tab carried over the pages let go of: laid out where it landed, it stays so (true);
    /// else nothing was laid out (false).
    pub(in super::super) fn land(&mut self) -> bool {
        self.drop = None;
        self.landing.take().is_some()
    }

    /// The line between the halves dragged `moved` points since the person took hold of it; let
    /// go of when `done`.
    pub(in super::super) fn drag_split(&mut self, moved: i32, done: bool) {
        let (size, ..) = self.area(false);
        let room = (size.width - super::super::split::GAP).max(1.0);
        let from = *self.split.grabbed.get_or_insert(self.split.share);
        self.split.share = super::super::split::held(from + f64::from(moved) / room, room);
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
        // Split, the window shows two tabs, each in its half; the one in use has the toolbar.
        let halves = if over {
            None
        } else {
            self.halves(size.width, left)
        };
        let raised = self.stack(halves.is_some());
        if let Some(strip) = &self.strip {
            let height = if self.menu.is_some() {
                top + size.height
            } else {
                top
            };
            pin(strip, LogicalSize::new(width, height));
        }
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
            // Over a whole page it stretches with the window as the window resizes; over one
            // half of a split it keeps its width until the layout that follows.
            if halves.is_none() {
                super::super::platform::follow(nav, super::super::platform::Follow::Top);
            }
        }
        // An open menu leaves the side in place: the strip, clear but for the menu, lies over it.
        self.place_side(size.height, top);
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
            // Out of sight, a terminal's output waits unrun; back in sight, what waited runs.
            if open.slot.shown(spot.is_some()) {
                let _ = self.proxy.send_event(Cmd::Paint(open.id));
            }
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

    /// The views over the pages, the last on top: the toolbar while its suggestions are open, the
    /// line while it shows (`split`, or a tab dragged over the pages), and the strip. Whether the
    /// strip lies over the pages.
    fn stack(&self, split: bool) -> bool {
        // Bottom to top over the pages: the browser's toolbar, then the line between the halves
        // (whole, over the toolbar of either half), then the strip. While the person types an
        // address, the toolbar and its suggestions lie over the line instead.
        let mut over: Vec<&wry::WebView> = Vec::with_capacity(4);
        let line = (self.line.as_ref()).filter(|_| split);
        if let (Some(nav), false) = (&self.nav, self.typing) {
            over.push(nav);
        }
        over.extend(line);
        if let (Some(nav), true) = (&self.nav, self.typing) {
            over.push(nav);
        }
        let Some(strip) = &self.strip else {
            return false;
        };
        over.push(strip);
        super::super::platform::stack(&over)
    }

    /// The line's view: over the half a tab dragged out over the pages would go into, else on the
    /// line between a split window's halves, else out of sight.
    fn place_line(
        &self,
        halves: Option<[(Key, f64, f64); 2]>,
        (size, _, top): (LogicalSize<f64>, f64, f64),
        width: f64,
    ) {
        if let Some(line) = &self.line {
            let at = halves.map(|[_, second]| second.1 - super::super::split::GAP);
            let x = at.map_or(-(width + 100.0), |at| {
                at + super::super::split::GAP / 2.0 - super::super::split::LINE / 2.0
            });
            let column = LogicalSize::new(super::super::split::LINE, size.height);
            let _ = line.set_bounds(super::super::frame::fill(column, x, top));
            // Its width and place kept as the window resizes, as tall as the window.
            super::super::platform::follow(line, super::super::platform::Follow::Side);
        }
    }

    /// The side open or shut, as the person asked, in one step: the side and the tabs beside it
    /// laid out together, so nothing on the way is drawn at a width it does not keep.
    pub(in super::super) fn slide_side(&mut self, shared: &Shared) {
        if self.side.is_none() {
            return;
        }
        self.side_open = !self.side_open;
        crate::lock(&shared.kept).side = Some(self.side_open);
        self.side_now = if self.side_open {
            self.side_width
        } else {
            RAIL
        };
        self.refresh(shared);
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
        self.say(strip, "offer", &super::super::offer::script(bar.as_ref()));
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
        // The split's two tabs, left and right: the side lists them as one; another tab shown,
        // the project's split still listed so.
        let on = self.pair().map(|(left, right)| [left.name(), right.name()]);
        let kept = (shared.arranged_of(&project).as_ref())
            .and_then(super::super::split::kept_of)
            .as_ref()
            .map(super::super::split::sides);
        let split = format!("SP({})", serde_json::json!(on.or(kept)));
        for view in [Some(strip), self.side.as_ref(), self.nav.as_ref()]
            .into_iter()
            .flatten()
        {
            self.say(view, "split", &split);
            self.say(view, "drawn", &drawn);
        }
        let rows = (self.menu == Some(Menu::History)).then(|| self.rows(shared));
        if self.menu != Some(Menu::Events) {
            let _ = strip.evaluate_script(&strip::menu(rows.as_deref()));
        }
        let events = crate::lock(&shared.center).clone();
        let center = strip::center(&events, self.menu == Some(Menu::Events));
        // The history and the Event Center draw into one list: each says what it shows, always.
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
        self.say(nav, "sizes", &super::super::sizes::script(framed, scale));
        let held = shared.comments.count();
        self.say(nav, "commenting", &strip::commenting(self.commenting, held));
        let (acting, waiting) = shared.turn.now();
        let turn = strip::Turn::of(acting, self.commenting, waiting);
        self.say(nav, "turn", &strip::turn(turn));
    }

    /// `script`, of `kind`, run in `view`, a part of the strip, unless it is what that part was
    /// last told of that kind (and no part has loaded its page again since): a refresh with
    /// nothing new wakes no page.
    fn say(&self, view: &wry::WebView, kind: &'static str, script: &str) {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        script.hash(&mut hasher);
        let hash = hasher.finish();
        let loads = super::LOADS.load(std::sync::atomic::Ordering::SeqCst);
        let mut said = crate::lock(&self.said);
        if said.0 != loads {
            *said = (loads, std::collections::HashMap::new());
        }
        let at = (std::ptr::from_ref(view) as usize, kind);
        if said.1.insert(at, hash) != Some(hash) {
            let _ = view.evaluate_script(script);
        }
    }
}
