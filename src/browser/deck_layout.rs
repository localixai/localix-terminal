//! The window's layout: the strip along the top, the tabs' column (the side) down the left under
//! it, what is shown beside that, the rest out of sight, and the strip and the side told what the
//! window holds. While the history menu is open the strip takes the whole window.

use tao::dpi::LogicalSize;

use super::super::host::Cmd;
use super::super::tabs::Tabs;

use super::super::host::Shared;

use super::super::strip::{BAR, HEIGHT, Key, NAV, RAIL, SIDE_MAX, SIDE_MIN};
use super::super::tabs::{pin, place};
use super::{Deck, Shown};

impl Deck {
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

    /// Whether a page is what the window shows (not a terminal, a board or Settings): what the
    /// app menu's page-only items are valid for.
    pub(in super::super) fn page_shown(&self) -> bool {
        self.shown == Shown::Web && self.web.is_some()
    }

    /// The side at its width, its rail when shut, `tall` points high from `top`.
    fn place_side(&self, tall: f64, top: f64) {
        let Some(side) = &self.side else {
            return;
        };
        let column = LogicalSize::new(self.side_wide(), tall);
        super::super::platform::bound(side, column, 0.0, top);
        super::super::platform::follow(side, super::super::platform::Follow::Side);
    }

    /// How wide the side is in this window: the width it keeps, held to half the window (never
    /// under `SIDE_MIN`), so a window narrower than the one the width was set in still has room
    /// for its pages. Its rail is as narrow as it is.
    pub(in super::super) fn side_wide(&self) -> f64 {
        let native = &self.frame.native;
        let window = native
            .inner_size()
            .to_logical::<f64>(native.scale_factor())
            .width;
        self.side_now.min((window / 2.0).max(SIDE_MIN))
    }

    /// How much of the window's left the side keeps now: the tabs lie beside it.
    fn left(&self) -> f64 {
        if self.side.is_some() {
            self.side_wide()
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
    pub(super) fn halves(&self, width: f64, left: f64) -> Option<[(Key, f64, f64); 2]> {
        let (first, second) = self.pair()?;
        // A share kept from a wider window, or the side opening, never leaves a half too narrow.
        let room = (width - super::super::split::GAP).max(1.0);
        let share = super::super::split::held(self.split.share, room);
        let ((x, w), (x2, w2)) = super::super::split::halves(width, left, share);
        Some([(first, x, w), (second, x2, w2)])
    }

    /// The tabs' part of the window: a page's under its toolbar (`nav`), a terminal's under the
    /// strip.
    pub(super) fn area(&self, nav: bool) -> (LogicalSize<f64>, f64, f64) {
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
            super::super::platform::bound(nav, LogicalSize::new(wide, tall), x, top);
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
            super::super::platform::bound(line, column, x, top);
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
}
