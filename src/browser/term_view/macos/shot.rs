//! A picture of the terminal for an agent (`screenshot` of a terminal's tab): its whole frame
//! drawn by the view's own canvas in the look it is shown in, not taken from the screen, so a
//! tab out of sight has one too; at most the screenshots' long side, as a page's picture is.

use std::sync::mpsc::SyncSender;

use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGImage;

use super::super::painter;
use super::view::TermView;
use crate::browser::capture::{self, Shot};

impl TermView {
    /// The picture of the terminal now, sent to `reply` as JPEG once encoded.
    pub(crate) fn shot(&self, reply: SyncSender<Result<Shot, String>>) {
        match self.picture() {
            Some(image) => capture::drawn(&image, reply),
            None => drop(reply.send(Err("the terminal's view drew no picture".into()))),
        }
    }

    /// The whole frame drawn: the view's own when it is shown (brought up to now first), else
    /// one asked of the terminal for the picture alone.
    fn picture(&self) -> Option<CFRetained<CGImage>> {
        let shown = self.state()?.shown;
        if shown {
            self.redraw(false);
        }
        let state = self.state()?;
        let shown = shown.then_some(&state.frame);
        painter::for_agent(state.painter.as_ref()?, shown, &*state.surface)
    }
}
