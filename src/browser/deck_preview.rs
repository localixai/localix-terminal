//! The deck's side of a tab carried over its panes (docs/windows.md): while the tab is aimed
//! at a landing, the window is laid out as that landing would lay it, with the tab's own view
//! standing where it would land and the panes beside it making room, each the view itself in
//! its place. Nothing is told to the layout meanwhile, and a terminal keeps its grid (held as
//! while a divider is dragged), so no program is told a size until the tab is let go. Aimed
//! elsewhere, or off the panes, the window is laid out again, once; the views go their way by
//! springs the system plays (`glide.rs`).

use std::time::Duration;

use tao::dpi::LogicalSize;

use super::super::glide::{self, Moved, Opens};
use super::super::panel::Face;
use super::super::split::GAP;
use super::super::strip::Key;
use super::super::term_view::{CarrySeen, PaneSeen};
use super::Deck;
use super::panes::Panes;
use crate::browser::Side;
use crate::layout::geometry::{Laid, Rect};

/// The carried tab standing where it would land: the panes as that landing lays them out,
/// the tab's among them, over the panes' part of the window as it was then.
pub(super) struct Preview {
    key: Key,
    laid: Laid<Key>,
    area: Rect,
}

/// What carrying a tab over the panes cost, for checks: how many times the window was laid
/// out for it, and the pointer's aims (how many, the longest and all of them, in microseconds
/// of the main thread's work).
#[derive(Default)]
pub(super) struct Cost {
    laid: u32,
    aims: u32,
    aim_most: u32,
    aim_all: u64,
}

impl Deck {
    /// The views of pane `key`: a page's, or a panel's (a terminal or a page).
    fn views_of(&self, key: Key) -> Vec<Moved<'_>> {
        let view = match key {
            Key::Web(id) => (self.web.as_ref())
                .and_then(|web| web.at(Some(id)))
                .map(Moved::Page),
            Key::Term(id) => self.term(id).map(|open| match &open.face {
                Face::Page(page) => Moved::Page(page),
                Face::Term(view) => Moved::Term(view),
            }),
        };
        view.into_iter().collect()
    }

    /// Every view a landing may move: each pane in sight, the pane of `also`, the browser's
    /// toolbar and the documents' row.
    fn moved(&self, also: Key) -> Vec<Moved<'_>> {
        let mut keys = self.sight();
        if !keys.contains(&also) {
            keys.push(also);
        }
        let mut views: Vec<Moved<'_>> = (keys.iter()).flat_map(|key| self.views_of(*key)).collect();
        views.extend(self.nav.iter().chain(&self.docs).map(Moved::Page));
        views
    }

    /// A tab lifted in the side: a divider's view more than the tree shown needs is made now,
    /// so none is made while the tab is carried over the panes.
    pub(in super::super) fn ready_to_land(&mut self) {
        let now = self
            .panes_in(self.area(false))
            .map_or(0, |panes| panes.dividers.len());
        self.lines_for(now + 1);
    }

    /// The carried tab `key` stands where it would land (`plan`: the panes as that lays them
    /// out, and the side of the pane beside it that it lands on), or nowhere. The window is
    /// laid out so, once, when that is not how it is laid out already; the tab's pane opens
    /// from the edge it lands at, its divider with it, and the others go from where they stand.
    pub(in super::super) fn stand(&mut self, key: Key, plan: Option<(&Laid<Key>, Side)>) {
        let same = match (&self.preview, plan) {
            (None, None) => true,
            (Some(now), Some((laid, _))) => now.key == key && now.laid == *laid,
            (None, Some(_)) | (Some(_), None) => false,
        };
        if same {
            return;
        }
        if let Some((laid, _)) = plan {
            self.lines_for(laid.dividers.len());
        }
        let stood = glide::standing(&self.moved(key));
        self.preview = plan.map(|(laid, _)| Preview {
            key,
            laid: laid.clone(),
            area: self.pages_rect(),
        });
        self.layout();
        crate::lock(&self.cost).laid += 1;
        let views = self.views_of(key);
        let opens = plan.map(|(laid, side)| Opens {
            views: &views,
            side,
            line: self.line_beside(laid, key, side),
        });
        stood.go(opens.as_ref());
    }

    /// The carried tab was let go where it stands: from now on the window is laid out from
    /// the layout's own tree, which the landing told it (`On::land`) makes the same.
    pub(in super::super) fn stands(&mut self) {
        self.preview = None;
    }

    /// Whether a carried tab stands where it would land now.
    pub(super) fn previewing(&self) -> bool {
        self.preview.is_some()
    }

    /// The panes the window draws over the tabs' part (`area`, from `Deck::area`): as the
    /// carried tab's landing lays them out while it stands there, the window's part the same
    /// and every pane of it still here; else the tree of the tab shown.
    pub(super) fn drawn_in(&self, area: (LogicalSize<f64>, f64, f64)) -> Option<Panes> {
        let (size, left, top) = area;
        let part = Rect::new(left, top, size.width, size.height);
        let standing = self.preview.as_ref().filter(|preview| {
            let here = |(key, _): &(Key, Rect)| self.there(*key);
            let shown = |(key, _): &(Key, Rect)| Some(*key) == self.shown_key();
            preview.area == part
                && preview.laid.panes.iter().all(here)
                && preview.laid.panes.iter().any(shown)
        });
        match standing {
            Some(preview) => Some(preview.laid.clone()),
            None => self.panes_in(area),
        }
    }

    /// The view of the divider between tab `key` and the pane it lands beside on `side`, as
    /// `laid` has them, and how far that pane's edge travels to make room for the tab.
    fn line_beside(&self, laid: &Laid<Key>, key: Key, side: Side) -> Option<(&wry::WebView, f64)> {
        let pane = laid.rect(&key)?;
        let near = |one: f64, other: f64| (one - other).abs() < 0.5;
        let across = side.across();
        let at = laid.dividers.iter().position(|divider| {
            let gap = divider.rect;
            let along = near(gap.start(!across), pane.start(!across))
                && near(gap.length(!across), pane.length(!across));
            let touches = match side.forward() {
                true => near(gap.start(across) + gap.length(across), pane.start(across)),
                false => near(gap.start(across), pane.start(across) + pane.length(across)),
            };
            divider.across == across && along && touches
        })?;
        Some((self.lines.get(at)?, pane.length(across) + GAP))
    }

    /// The pointer aimed over the panes, which took the main thread `took`.
    pub(in super::super) fn aimed(&self, took: Duration) {
        let took = u32::try_from(took.as_micros()).unwrap_or(u32::MAX);
        let mut cost = crate::lock(&self.cost);
        cost.aims += 1;
        cost.aim_most = cost.aim_most.max(took);
        cost.aim_all += u64::from(took);
    }

    /// For checks: the carried tab standing where it would land, each pane in sight where its
    /// view goes and where it is drawn now, and what carrying cost.
    pub(in super::super) fn carry_seen(&self) -> CarrySeen {
        let cost = crate::lock(&self.cost);
        CarrySeen {
            standing: self.preview.as_ref().map(|preview| preview.key.name()),
            panes: self.panes_seen(),
            laid: cost.laid,
            aims: cost.aims,
            aim_most: cost.aim_most,
            aim_all: cost.aim_all,
        }
    }

    /// Each pane in sight as its own view is drawn (macOS, where the system plays its way).
    #[cfg(target_os = "macos")]
    fn panes_seen(&self) -> Vec<PaneSeen> {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a place in a window, whole points"
        )]
        let whole = |part: f64| part.round() as i32;
        let seen = |key: Key| {
            let view = self.views_of(key).into_iter().next()?;
            let drawn = glide::drawn(view)?;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "an opacity in hundredths, 0 to 100"
            )]
            let opacity = (drawn.opacity * 100.0).round().clamp(0.0, 100.0) as u8;
            Some(PaneSeen {
                key: key.name(),
                rect: drawn.goes.map(whole),
                now: drawn.now.map(whole),
                holds: [whole(drawn.holds.0), whole(drawn.holds.1)],
                raised: whole(drawn.raised),
                opacity,
                playing: drawn.playing,
                seen: drawn.seen,
            })
        };
        self.sight().into_iter().filter_map(seen).collect()
    }

    /// Elsewhere nothing is read of a pane's view.
    #[cfg(not(target_os = "macos"))]
    #[expect(clippy::unused_self, reason = "macOS's, with no view read here")]
    fn panes_seen(&self) -> Vec<PaneSeen> {
        vec![]
    }
}
