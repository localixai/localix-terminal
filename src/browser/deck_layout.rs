//! The window's layout: the strip along the top, the tabs' column (the side) down the left under
//! it, what is shown beside that, the rest out of sight, and the strip and the side told what the
//! window holds. While the history menu is open the strip takes the whole window.

use tao::dpi::LogicalSize;

use super::super::tabs::Tabs;

use super::super::host::Shared;

use super::super::strip::{BAR, HEIGHT, Key, RAIL, side_held, side_widest};
use super::super::tabs::pin;
use super::{Deck, Shown};
use crate::layout::geometry::Rect;

impl Deck {
    /// The keys go to the tab shown, after the person switched to it themselves (with no tab
    /// shown, to the start of a new one): never while the window is not the key window (an
    /// agent's switch, or another app in front), while the person types in the strip, or while
    /// a menu is open.
    pub(in super::super) fn focus_shown(&self) {
        if self.typing || self.covered() || !self.frame.is_key() {
            return;
        }
        let view = match self.shown {
            Shown::Web => self.web.as_ref().map(Tabs::active),
            Shown::Term(id) => match self.term(id) {
                Some(open) => return open.face.focus(),
                None => None,
            },
            Shown::Nothing => None,
        };
        // No tab shown: the start of a new one takes them, never a tab out of sight.
        let view = view.or_else(|| self.nav.as_ref().filter(|_| self.blank()));
        if let Some(view) = view {
            let _ = view.focus();
        }
    }

    /// Whether the window can show a dialog (dialog.rs): it has a strip to draw it, and is on
    /// screen. Away, a question is asked as the system asks.
    pub(in super::super) fn dialogs(&self) -> bool {
        let native = &self.frame.native;
        self.strip.is_some() && native.is_visible() && !native.is_minimized()
    }

    /// The views under a dialog kept still while it is up (hush.rs), and let go after.
    fn hush(&self) {
        let on = self.strip.is_some() && super::super::dialog::up();
        let pages = self.web.as_ref().map(Tabs::active);
        for open in &self.terms {
            open.face.hush(on);
        }
        for view in pages
            .into_iter()
            .chain(&self.side)
            .chain(&self.nav)
            .chain(&self.docs)
        {
            super::super::hush::set(view, on);
        }
    }

    /// Whether the strip has the whole window: a menu is open, or a dialog is up.
    pub(super) fn covered(&self) -> bool {
        self.menu.is_some() || (self.strip.is_some() && super::super::dialog::up())
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
        let (x, top) = self.pane_origin(self.shown_key());
        (x, top + self.docs_over(self.shown_key()))
    }

    /// Where the pane of tab `key` begins in the window: the top left of its place in a split
    /// tab, else of the tabs' whole part.
    pub(super) fn pane_origin(&self, key: Option<Key>) -> (f64, f64) {
        let (_, left, top) = self.area(false);
        let rect = key.and_then(|key| self.panes()?.rect(&key));
        rect.map_or((left, top), |rect| (rect.x, rect.y))
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

    /// How wide the side is in this window: the width it keeps, held to the widest it may be
    /// here (half the window, never under `SIDE_MIN`), so a window narrower than the one the
    /// width was set in still has room for its pages. Its rail is as narrow as it is.
    pub(in super::super) fn side_wide(&self) -> f64 {
        let native = &self.frame.native;
        let window = native
            .inner_size()
            .to_logical::<f64>(native.scale_factor())
            .width;
        self.side_now.min(side_widest(Some(window)))
    }

    /// How much of the window's left the side keeps now: the tabs lie beside it.
    fn left(&self) -> f64 {
        self.side.as_ref().map_or(0.0, |_| self.side_wide())
    }

    /// The shown tab's part of the window, under the strip (and a page's toolbar) and beside the
    /// side, its place in a split one: its size, and how far from the window's left and top it
    /// starts.
    pub(super) fn content(&self) -> (LogicalSize<f64>, f64, f64) {
        let browsing = self.browsing();
        let (size, left, top) = self.area(browsing);
        let now = self.shown_key();
        // A document's pane starts under the row of its session's documents.
        let docs = self.docs_over(now);
        let place = now.and_then(|key| self.drawn_in(self.area(false))?.rect(&key));
        match place {
            Some(rect) => {
                let above = docs + if browsing { self.toolbar() } else { 0.0 };
                let tall = (rect.height - above).max(0.0);
                (LogicalSize::new(rect.width, tall), rect.x, rect.y + above)
            }
            None => (
                LogicalSize::new(size.width, (size.height - docs).max(0.0)),
                left,
                top + docs,
            ),
        }
    }

    /// The tabs' part of the window: a page's under its toolbar (`nav`, with the page's find row
    /// while it is open), a terminal's under the strip.
    pub(super) fn area(&self, nav: bool) -> (LogicalSize<f64>, f64, f64) {
        let native = &self.frame.native;
        let size = native.inner_size().to_logical::<f64>(native.scale_factor());
        let left = self.left();
        let top = self.top() + if nav { self.toolbar() } else { 0.0 };
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
        let over = self.covered() || self.typing;
        // Split, the window shows the shown tab's tree, each pane in its place (while a carried
        // tab stands where it would land, as that landing lays them out); the one in use has
        // the toolbar. All stay under a list the strip opens over them.
        let split = self.drawn_in((size, left, top));
        let raised = self.stack(split.as_ref().map_or(0, |panes| panes.dividers.len()));
        let panes = super::super::split::under(split, over, raised);
        if let Some(strip) = &self.strip {
            let height = if self.covered() {
                top + size.height
            } else {
                top
            };
            pin(strip, LogicalSize::new(width, height));
        }
        let in_use = self.shown_key();
        let used = in_use.and_then(|key| panes.as_ref()?.rect(&key));
        self.place_nav(used, (left, top, size), panes.is_none());
        // An open menu leaves the side in place: the strip, clear but for the menu, lies over it.
        self.place_side(size.height, top);
        // Where the strip lies over the pages, they stay under an open menu.
        let over = over && !raised;
        if let Some(web) = &self.web {
            let toolbar = self.toolbar();
            let pages: Vec<super::super::tabs::Pane> = match &panes {
                Some(panes) => (panes.panes.iter())
                    .filter_map(|(key, rect)| match key {
                        Key::Web(id) => {
                            let above = if Some(*key) == in_use { toolbar } else { 0.0 };
                            let above = above + self.docs_over(Some(*key));
                            let tall = (rect.height - above).max(0.0);
                            Some((
                                *id,
                                LogicalSize::new(rect.width, tall),
                                rect.x,
                                rect.y + above,
                            ))
                        }
                        Key::Term(_) => None,
                    })
                    .collect(),
                None if self.shown == Shown::Web && !over => {
                    let (size, left, top) = self.area(self.nav.is_some());
                    let docs = self.docs_over(in_use);
                    let size = LogicalSize::new(size.width, (size.height - docs).max(0.0));
                    vec![(web.active_id(), size, left, top + docs)]
                }
                None => vec![],
            };
            let (whole, _, under) = self.area(self.nav.is_some());
            web.layout(&pages, whole, under);
        }
        // A divider held, or a carried tab standing where it would land: each terminal keeps
        // its grid until it is let go (`TermView::hold`).
        let held = self.dragging() || self.previewing();
        for open in &self.terms {
            open.face.hold(held);
            let key = Key::Term(open.id);
            let spot = match &panes {
                Some(panes) => panes.rect(&key),
                None => (self.shown == Shown::Term(open.id) && !over).then_some(Rect::new(
                    left,
                    top,
                    size.width,
                    size.height,
                )),
            };
            let docs = self.docs_over(Some(key));
            let row = self.find_under(open.id);
            match spot {
                Some(rect) => open.face.place(
                    rect.x,
                    rect.y + docs,
                    LogicalSize::new(rect.width, (rect.height - docs - row).max(0.0)),
                    true,
                ),
                None => open.face.place(-(width + 100.0), top, size, false),
            }
        }
        self.place_lines(panes.as_ref(), width);
        self.place_docs(panes.as_ref(), (left, size.width, top), over);
        self.hush();
    }

    /// The browser's toolbar placed: over the page shown (in its pane, `used`, when the tab is
    /// split: `whole_tab` false), across the window with no tab shown, or the find row alone
    /// under a terminal; else out of sight. `size` is the tabs' part, from `left` and `top`.
    fn place_nav(
        &self,
        used: Option<Rect>,
        (left, top, size): (f64, f64, LogicalSize<f64>),
        whole_tab: bool,
    ) {
        let width = size.width + left;
        let in_use = self.shown_key();
        let Some(nav) = &self.nav else {
            return;
        };
        // Over a page, its toolbar; under a terminal, the find row alone, while it is open.
        let whole = Rect::new(left, top, size.width, size.height);
        let row = self.term_row(used, whole);
        let blank = self.blank();
        let x = if self.browsing() || blank {
            used.map_or(left, |rect| rect.x)
        } else {
            -(width + 100.0)
        };
        let tall = if self.typing || blank {
            size.height
        } else {
            self.toolbar()
        };
        let wide = match (blank, used) {
            (false, Some(rect)) => rect.width,
            _ => size.width,
        };
        let under = used.map_or(top, |rect| rect.y) + self.docs_over(in_use);
        let at = row.unwrap_or_else(|| Rect::new(x, under, wide, tall));
        super::super::platform::bound(nav, LogicalSize::new(at.width, at.height), at.x, at.y);
        // Over a whole page it stretches with the window as the window resizes; over one
        // pane of a split, or under a terminal, it keeps its place until the layout after.
        let follow = match (whole_tab, row.is_none()) {
            (_, false) => Some(super::super::platform::Follow::Still),
            (true, true) => Some(super::super::platform::Follow::Top),
            (false, true) => None,
        };
        if let Some(follow) = follow {
            super::super::platform::follow(nav, follow);
        }
    }

    /// The views over the pages, the last on top: the toolbar while its suggestions are open,
    /// the dividers of the tree shown (`lines` of them), and the strip. Whether the strip lies
    /// over the pages.
    fn stack(&self, lines: usize) -> bool {
        // Bottom to top over the pages: the documents' row, the browser's toolbar, then the
        // dividers (over the toolbar of a pane beside them), then the strip. While the person
        // types an address, the toolbar and its suggestions lie over the dividers instead.
        let mut over: Vec<&wry::WebView> = Vec::with_capacity(4 + lines);
        over.extend(&self.docs);
        if let (Some(nav), false) = (&self.nav, self.typing) {
            over.push(nav);
        }
        over.extend(self.lines_shown(lines));
        if let (Some(nav), true) = (&self.nav, self.typing) {
            over.push(nav);
        }
        let Some(strip) = &self.strip else {
            return false;
        };
        over.push(strip);
        super::super::platform::stack(&over)
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
        let width = side_held(width, Some(window)).round();
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
