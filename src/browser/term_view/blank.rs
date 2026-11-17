//! A terminal's view where the window draws none (a system other than macOS and Linux): it
//! stands where the terminal's pane is and shows nothing, and the terminal runs on, read by
//! agents.

use std::sync::Arc;

use tao::dpi::LogicalSize;

use crate::surface::Surface;

/// A view that draws nothing, holding its terminal while the tab is open.
pub(crate) struct TermView {
    surface: Arc<dyn Surface>,
}

impl TermView {
    pub(crate) fn open(
        _native: &tao::window::Window,
        surface: Arc<dyn Surface>,
        _opens: Box<dyn Fn(&str)>,
    ) -> Option<Self> {
        Some(Self { surface })
    }

    pub(crate) fn find(&self, _step: &crate::surface::FindStep) -> crate::surface::Count {
        crate::surface::Count::default()
    }

    /// What its program copied and whether it rang: taken, so neither waits for a view.
    pub(crate) fn told(&self) -> (Option<String>, bool) {
        (self.surface.copied(), self.surface.rang())
    }

    pub(crate) fn place(&self, _x: f64, _top: f64, _size: LogicalSize<f64>, _shown: bool) {}

    pub(crate) fn focus(&self) {}

    pub(crate) fn hold(&self, _on: bool) {}

    pub(crate) fn has_keys(&self) -> bool {
        false
    }

    pub(crate) fn reparent(&self, _native: &tao::window::Window) {}

    pub(crate) fn hush(&self, _on: bool) {}

    pub(crate) fn draw(&self) {}

    pub(crate) fn restyle(&self) {}

    /// No view is ever in a window here.
    pub(crate) const fn placed_in(_native: &tao::window::Window) -> usize {
        0
    }

    pub(crate) fn probe(&self, _probe: super::TermProbe) -> Option<super::TermAnswer> {
        None
    }

    /// No picture: the terminal is read as text here.
    pub(crate) fn shot(
        &self,
        reply: std::sync::mpsc::SyncSender<Result<crate::browser::Shot, String>>,
    ) {
        drop(reply.send(Err(
            "a terminal has a picture on macOS and Linux only; its text is below".into(),
        )));
    }
}
