//! What a panel shows: the page its caller gives (the board, the Store, Settings), or a terminal
//! drawn by the window itself from its `Surface` (`term_view`). The window places, shows,
//! focuses and moves either one the same way; only a page runs scripts.

use std::sync::Arc;

use tao::dpi::LogicalSize;
use wry::WebView;

use super::super::term_view::TermView;
use crate::surface::Surface;

/// What a panel opens with: a page's markup, or the terminal it draws.
pub(in crate::browser) enum Body {
    Page(String),
    Term(Arc<dyn Surface>),
}

/// A panel's view in the window (a page's held apart: a WebKitGTK view is several kilobytes).
pub(in crate::browser) enum Face {
    Page(Box<WebView>),
    #[cfg(target_os = "macos")]
    Term(super::super::term_view::Owned),
    #[cfg(not(target_os = "macos"))]
    Term(TermView),
}

impl Face {
    /// The panel's page, when it shows one.
    pub(in crate::browser) fn page(&self) -> Option<&WebView> {
        match self {
            Self::Page(page) => Some(&**page),
            Self::Term(_) => None,
        }
    }

    /// The terminal it draws, when it draws one.
    pub(in crate::browser) fn term(&self) -> Option<&TermView> {
        match self {
            Self::Page(_) => None,
            Self::Term(view) => Some(view),
        }
    }

    /// `size` large, `x` across and `top` down its window, or out of sight.
    pub(in crate::browser) fn place(&self, x: f64, top: f64, size: LogicalSize<f64>, shown: bool) {
        match self {
            Self::Page(page) => super::super::tabs::place(page, x, top, size, shown),
            Self::Term(view) => view.place(x, top, size, shown),
        }
    }

    /// Whether a divider of its split is held, said before it is placed: a terminal keeps its
    /// grid meanwhile, and its place tells it the one that fits once let go (a page fits itself).
    pub(in crate::browser) fn hold(&self, on: bool) {
        if let Self::Term(view) = self {
            view.hold(on);
        }
    }

    pub(in crate::browser) fn focus(&self) {
        match self {
            Self::Page(page) => drop(page.focus()),
            Self::Term(view) => view.focus(),
        }
    }

    /// Whether it holds its window's keys.
    pub(in crate::browser) fn has_keys(&self) -> bool {
        match self {
            Self::Page(page) => super::super::platform::has_keys(page),
            Self::Term(view) => view.has_keys(),
        }
    }

    /// Moved into `native`, as it is.
    pub(in crate::browser) fn reparent(&self, native: &tao::window::Window) {
        match self {
            Self::Page(page) => super::super::platform::reparent(page, native),
            Self::Term(view) => view.reparent(native),
        }
    }

    /// Kept still under the window's dialog (`on`), and let go after.
    pub(in crate::browser) fn hush(&self, on: bool) {
        match self {
            Self::Page(page) => super::super::hush::set(page, on),
            Self::Term(view) => view.hush(on),
        }
    }
}
