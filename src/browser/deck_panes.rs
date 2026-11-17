//! The shown tab's tree in this window (split.rs): which tree is drawn, where each of its panes
//! lies by the layout's geometry, the views of its dividers (a pool of the strip's page, one
//! per divider, made as a tree needs them and put out of sight when it needs fewer), and which
//! pane lies under a point.

use tao::dpi::LogicalSize;

use super::super::split::{self, LINE, SPACING};
use super::super::strip::Key;
use super::super::{Arranging, Drawn, platform};
use super::{Deck, Part};
use crate::layout::geometry::{self, Laid, Rect};

/// A tree laid out in the window, its panes by their keys.
pub(in super::super) type Panes = Laid<Key>;

impl Deck {
    /// Each tab of the window's projects kept as the layout has them now: what the window draws
    /// a shown pane's tab from.
    pub(super) fn keep_tabs(&self, tabs: Vec<Arranging>) {
        *crate::lock(&self.tabs) = tabs;
    }

    /// The tab the layout keeps pane `key` in, in sight or put by.
    pub(in super::super) fn tab_holding(&self, key: &str) -> Option<Arranging> {
        let tabs = crate::lock(&self.tabs);
        let holds = |tab: &&Arranging| tab.all.iter().any(|pane| pane == key);
        tabs.iter().find(holds).cloned()
    }

    /// Whether the layout takes tab `beside` put beside tab `shown` (session.rs).
    pub(in super::super) fn takes(&self, shown: Key, beside: Key) -> bool {
        super::super::session::takes(&crate::lock(&self.tabs), shown, beside)
    }

    /// The tree the window draws for the tab shown, by its panes' keys: a pane that is not a tab
    /// of this window left out. None while it shows a pane alone.
    pub(in super::super) fn tree(&self) -> Option<Drawn> {
        let shown = self.shown_key()?.name();
        let tree = self.split.tree(self.tab_holding(&shown).as_ref(), &shown)?;
        let there = |key: &String| {
            Key::parse(key)
                .filter(|key| self.there(*key))
                .map(|_| key.clone())
        };
        tree.named(&there).filter(|tree| tree.visible().len() >= 2)
    }

    /// The shown pane's tab as the layout keeps it, zoomed or not: its tree with that pane in
    /// sight, a pane that is not a tab of this window left out. None for a pane alone in it.
    pub(in super::super) fn tab_tree(&self) -> Option<Drawn> {
        let shown = self.shown_key()?.name();
        let mut tree = self.tab_holding(&shown)?.tree;
        tree.show(&shown);
        let there = |key: &String| {
            Key::parse(key)
                .filter(|key| self.there(*key))
                .map(|_| key.clone())
        };
        tree.named(&there).filter(|tree| tree.visible().len() >= 2)
    }

    /// The tree the window draws laid over the tabs' part of the window (`area`, from
    /// `Deck::area`), none while it shows a pane alone.
    pub(in super::super) fn panes_in(
        &self,
        (size, left, top): (LogicalSize<f64>, f64, f64),
    ) -> Option<Panes> {
        let tree = self.tree()?.named(&|key: &String| Key::parse(key))?;
        let area = Rect::new(left, top, size.width, size.height);
        Some(geometry::lay(&tree, area, SPACING))
    }

    /// The panes laid out as the window shows them now (the tree of the tab shown, or a
    /// carried tab's landing while it stands there): none while a list it must stand aside
    /// for is open over the pages.
    pub(in super::super) fn panes(&self) -> Option<Panes> {
        let raised = platform::OVER_PAGES;
        split::under(
            self.drawn_in(self.area(false)),
            self.covered() || self.typing,
            raised,
        )
    }

    /// The tree of the tab shown laid out as the layout keeps it, whatever a carried tab
    /// shows meanwhile: none while a list it must stand aside for is open over the pages.
    pub(in super::super) fn panes_kept(&self) -> Option<Panes> {
        let raised = platform::OVER_PAGES;
        split::under(
            self.panes_in(self.area(false)),
            self.covered() || self.typing,
            raised,
        )
    }

    /// The panes in sight, the one shown alone when it is not split.
    pub(in super::super) fn sight(&self) -> Vec<Key> {
        match self.panes() {
            Some(panes) => panes.panes.into_iter().map(|(key, _)| key).collect(),
            None => self.shown_key().into_iter().collect(),
        }
    }

    /// The pane at `x`, `y` of the window (points from its top left), when the window is split
    /// and the point is on a pane: none on a divider, which a press takes hold of.
    pub(in super::super) fn pane_at(&self, x: f64, y: f64) -> Option<Key> {
        let panes = self.panes()?;
        let on_line = (panes.dividers.iter()).any(|divider| {
            let middle =
                divider.rect.start(divider.across) + divider.rect.length(divider.across) / 2.0;
            let reach = divider.rect.length(!divider.across);
            let along = if divider.across { y } else { x };
            let from = divider.rect.start(!divider.across);
            let off = if divider.across { x } else { y } - middle;
            off.abs() <= LINE / 2.0 && along >= from && along < from + reach
        });
        if on_line {
            return None;
        }
        (panes.panes.iter())
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(key, _)| *key)
    }

    /// One divider's view for each divider of the tree the window may draw now, made on the
    /// window's loop before they are laid out (a view is never made during a drag's frame).
    pub(in super::super) fn lines_enough(&mut self) {
        let wanted = self
            .panes_in(self.area(false))
            .map_or(0, |panes| panes.dividers.len());
        self.lines_for(wanted);
    }

    /// At least `wanted` dividers' views, the ones missing made now.
    pub(super) fn lines_for(&mut self, wanted: usize) {
        if self.side.is_none() {
            return;
        }
        while self.lines.len() < wanted {
            let part = Part::Line(self.lines.len());
            match super::construction::strip_page(
                &self.frame.native,
                self.look,
                self.proxy.clone(),
                part,
            ) {
                Ok(line) => self.lines.push(line),
                Err(why) => return log::warn!("a divider's view was not made: {why}"),
            }
        }
    }

    /// Each divider's view over its divider, the width of a hold either side of the drawn
    /// point and as long as the divider; the views the tree does not need out of sight. Each
    /// is told which way it moves.
    pub(super) fn place_lines(&self, panes: Option<&Panes>, width: f64) {
        let dividers = panes.map_or(&[][..], |panes| panes.dividers.as_slice());
        for (at, line) in self.lines.iter().enumerate() {
            let Some(divider) = dividers.get(at) else {
                let away = LogicalSize::new(LINE, 1.0);
                platform::bound(line, away, -(width + 100.0), 0.0);
                continue;
            };
            let rect = divider.rect;
            let (size, x, y) = if divider.across {
                let middle = rect.x + rect.width / 2.0;
                (
                    LogicalSize::new(LINE, rect.height),
                    middle - LINE / 2.0,
                    rect.y,
                )
            } else {
                let middle = rect.y + rect.height / 2.0;
                (
                    LogicalSize::new(rect.width, LINE),
                    rect.x,
                    middle - LINE / 2.0,
                )
            };
            platform::bound(line, size, x, y);
            // A divider across the tab keeps its place and grows as tall as the window as it
            // resizes, as the side does; one between panes one above the other waits for the
            // layout that follows.
            let follow = match divider.across {
                true => platform::Follow::Side,
                false => platform::Follow::Still,
            };
            platform::follow(line, follow);
            self.say((Part::Line(at), line), "way", &way(divider.across));
        }
    }

    /// The divider views over the pages, in their order.
    pub(super) fn lines_shown(&self, count: usize) -> impl Iterator<Item = &wry::WebView> {
        self.lines.iter().take(count)
    }
}

/// What a divider's page is told of the way it moves (`across`: between panes side by side),
/// with its name and orientation for assistive tools. A view of the pool is told again as the
/// divider it lies over changes, so the script keeps what it names to itself: a name declared
/// in the page's own scope could not be declared a second time, and nothing of it would run.
fn way(across: bool) -> String {
    let (way, oriented, named) = match across {
        true => ("across", "vertical", "Split width"),
        false => ("down", "horizontal", "Split height"),
    };
    format!(
        "{{document.documentElement.dataset.way='{way}';\
         const l=document.getElementById('line');\
         l.setAttribute('aria-orientation','{oriented}');l.setAttribute('aria-label','{named}')}}"
    )
}

#[cfg(test)]
#[path = "deck_panes_tests.rs"]
mod tests;
