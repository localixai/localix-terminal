//! What the work thread asks of the window for a terminal: its tab, drawn by the window itself
//! from the terminal (`term_view`), and, for checks, what its view shows and does.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use super::super::panel::{Body, Panel};
use super::super::term_view::{TermAnswer, TermProbe};
use super::{Browser, Cmd, Shot};
use crate::surface::Surface;

impl Browser {
    /// Terminal `id`'s tab titled `title`, the window drawing `surface` in it, shown once it
    /// opens; it closes when the panel is dropped. Its caller wakes it (`Panel::waker`) as the
    /// terminal's screen changes. It opens the window when the browser has not.
    pub fn term(&self, id: u32, title: &str, surface: Arc<dyn Surface>) -> Panel {
        let proxy = self.proxy.clone();
        let panel = Panel::new(id, Box::new(move |cmd| proxy.send_event(cmd).is_ok()));
        let slot = Arc::clone(&panel.slot);
        let open = Cmd::Panel((id, title.to_string(), Body::Term(surface), slot));
        if self.proxy.send_event(open).is_err() {
            panel.slot.alive.store(false, Ordering::SeqCst);
        }
        panel
    }

    /// A picture of terminal `id` as its view draws it, drawn by the window from the terminal
    /// (not taken from the screen: a tab out of sight has one too), as JPEG.
    pub fn term_shot(&self, id: u32) -> Result<Shot, String> {
        let (reply, answer) = mpsc::sync_channel(1);
        let asked = self.proxy.send_event(Cmd::TermShot(id, reply));
        asked.map_err(|gone| format!("the window is gone: {gone}"))?;
        let waited = answer.recv_timeout(std::time::Duration::from_secs(10));
        waited.map_err(|late| format!("the window drew no picture of t{id}: {late}"))?
    }

    /// For checks: what terminal `id`'s view is told or shows; `None` with no such view.
    #[doc(hidden)]
    pub fn term_probe(&self, id: u32, probe: TermProbe) -> Option<TermAnswer> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy
            .send_event(Cmd::TermProbe(id, probe, reply))
            .ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }
}
