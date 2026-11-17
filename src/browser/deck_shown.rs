//! Which tab the window shows: one tab, or none (the project shown has none open), and how that
//! is kept true as tabs come and go. Which project the tab is of is the host's to keep
//! (`On::settle`, host_project.rs): the deck holds every project's tabs and shows one.

use super::super::panel::Held;
use super::super::strip::Key;
use super::super::term_view::{self, TermProbe, TermView, TermViews};
use super::{Deck, Shown};

impl Deck {
    /// Whether no tab is shown: every one closed, or the project shown has none open. The
    /// window stays, and the browser's toolbar takes the pages' part as the start of a new tab.
    pub(super) fn blank(&self) -> bool {
        match self.shown {
            Shown::Web => self.web.is_none(),
            Shown::Term(id) => self.term(id).is_none_or(|open| open.held != Held::Live),
            Shown::Nothing => true,
        }
    }

    /// Show `shown`, if it is there.
    pub(in super::super) fn show(&mut self, shown: Shown) {
        let there = match shown {
            Shown::Web => self.web.is_some(),
            Shown::Term(id) => self.term(id).is_some(),
            Shown::Nothing => true,
        };
        if there {
            self.shown = shown;
        }
    }

    /// No tab shown: the project the window went to has none open. Every tab it holds stays
    /// out of sight, as it was; an open menu closes, as it does when a tab is shown.
    pub(in super::super) fn show_nothing(&mut self) {
        self.menu = None;
        self.shown = Shown::Nothing;
    }

    /// The tab shown, as the strip names it: none while an ended screen shows.
    pub(in super::super) fn shown_key(&self) -> Option<Key> {
        match self.shown {
            Shown::Web => self.web.as_ref().map(|web| Key::Web(web.active_id())),
            Shown::Term(id) => (self.term(id))
                .filter(|open| open.held == Held::Live)
                .map(|_| Key::Term(id)),
            Shown::Nothing => None,
        }
    }

    /// Which of the window's views holds its keys (`platform::has_keys`): the strip's parts by
    /// name, a page or a panel by its tab's key.
    pub(in super::super) fn keys_held(&self) -> Option<String> {
        let parts = [
            ("nav", &self.nav),
            ("strip", &self.strip),
            ("side", &self.side),
        ];
        let part = parts.into_iter().find_map(|(name, view)| {
            (view.as_ref()).and_then(|view| super::super::platform::has_keys(view).then_some(name))
        });
        if let Some(part) = part {
            return Some(part.into());
        }
        let page = (self.web.as_ref())
            .filter(|web| super::super::platform::has_keys(web.active()))
            .map(|web| Key::Web(web.active_id()));
        let panel = || {
            (self.terms.iter())
                .find(|open| open.face.has_keys())
                .map(|open| Key::Term(open.id))
        };
        page.or_else(panel).map(|key| key.name())
    }

    /// Whether tab `key` is open in the window.
    pub(super) fn there(&self, key: Key) -> bool {
        match key {
            Key::Web(id) => (self.web.as_ref()).is_some_and(|web| web.has(id)),
            Key::Term(id) => self.term(id).is_some_and(|open| open.held == Held::Live),
        }
    }

    /// What is shown is still a tab: else the newest terminal's, else the browser. (An ended
    /// screen the history menu showed is let go of here too.) Which project that tab is of is
    /// the host's to keep (`On::settle`); a window that shows none stays so.
    pub(super) fn settle(&mut self) {
        let gone = match self.shown {
            Shown::Web => self.web.is_none(),
            Shown::Term(id) => self.term(id).is_none_or(|open| open.held != Held::Live),
            Shown::Nothing => false,
        };
        if gone {
            let live = self.terms.iter().rev().find(|open| open.held == Held::Live);
            self.shown = match live {
                Some(open) => Shown::Term(open.id),
                None => Shown::Web,
            };
        }
    }

    /// For checks: the terminal views the window's tabs hold, the ones in its hierarchy, and
    /// the ones alive in the process.
    pub(in super::super) fn term_views(&self) -> TermViews {
        TermViews {
            held: (self.terms.iter())
                .filter(|open| open.face.term().is_some())
                .count(),
            placed: TermView::placed_in(&self.frame.native),
            alive: term_view::alive(),
        }
    }

    /// For checks: the window's edge taken hold of or let go, as the system tells each of its
    /// terminal views; whether it has one.
    pub(in super::super) fn edge_held(&self, probe: &TermProbe) -> bool {
        let views = self.terms.iter().filter_map(|open| open.face.term());
        views
            .map(|view| view.probe(probe.clone()).is_some())
            .fold(false, |any, one| any | one)
    }

    /// For checks: the window's content made `width` by `height` points, as a person's drag of
    /// its edge makes it: its views follow the window by their own rules, and it is laid out
    /// again, as it is when the system says it resized (which comes later, and lays it out once
    /// more to the same places). The size it had, in whole points.
    pub(in super::super) fn sized(&self, width: f64, height: f64) -> (u32, u32) {
        let was: tao::dpi::LogicalSize<u32> =
            (self.frame.native.inner_size()).to_logical(self.frame.native.scale_factor());
        // AppKit's own, at once (tao's waits for a later turn of the loop).
        #[cfg(target_os = "macos")]
        {
            use objc2::msg_send;
            let size = objc2_core_foundation::CGSize::new(width, height);
            let window = super::super::appkit::native(&self.frame.native);
            // SAFETY: an NSWindow takes the size of its content, a CGSize.
            let _: () = unsafe { msg_send![window, setContentSize: size] };
        }
        #[cfg(not(target_os = "macos"))]
        (self.frame.native).set_inner_size(tao::dpi::LogicalSize::new(width, height));
        self.layout();
        (was.width, was.height)
    }
}
