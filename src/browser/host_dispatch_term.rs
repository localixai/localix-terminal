//! What one window does for a terminal's view when its terminal or the work thread asks: draw
//! its newest frame (with what its program asked of the window as it drew, and a find's count
//! told again as new output is looked in), open a link the person Command-clicked in it, a
//! picture of it for an agent, and, for checks, what it shows or is told.

use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use crate::browser::capture::Shot;
use crate::browser::panel;
use crate::browser::strip::Key;
use crate::browser::term_links::{self, Clicked, Place};
use crate::browser::term_view::{TermAnswer, TermProbe};
use crate::browser::{Asked, Heard, platform};

use super::super::super::{Cmd, Shared};
use super::super::On;

/// An address the person opened from panel `id`'s text: a page asked of the app as any the
/// person asks for, in that panel's project, so it joins the session they look at and opens
/// where the window has no page yet.
pub(super) fn in_tab(shared: &Shared, id: u32, url: String) {
    let project = crate::lock(&shared.groups)
        .get(&Key::Term(id).name())
        .cloned();
    if let Some(project) = project {
        shared.heard(&project, Heard::Asked(Asked::Page(Some(url))));
    }
}

/// `url` opened in `place`: a tab of terminal `id`'s project, or the person's default browser.
fn open_in(place: Place, shared: &Shared, id: u32, url: String) {
    match place {
        Place::Tab => in_tab(shared, id, url),
        Place::Browser => {
            if !platform::open_outside(&url) {
                log::warn!("window: the system did not open {url}");
            }
        }
    }
}

impl On<'_> {
    /// A web address the person Command-clicked in terminal `id`: opened where Settings ›
    /// Terminal › Open links says, or, while it says Ask, where they answer the window's dialog.
    pub(super) fn term_link(&mut self, id: u32, url: String) {
        let setting = crate::browser::settings::now().open_links;
        match term_links::clicked(setting, &url) {
            Some(Clicked::Open(place)) => open_in(place, self.shared, id, url),
            Some(Clicked::Ask) => {
                let shared = Arc::clone(self.shared);
                self.choose(&term_links::question(&url), move |answer| {
                    if let Some(place) = term_links::answered(answer) {
                        open_in(place, &shared, id, url);
                    }
                });
            }
            None => {}
        }
    }

    /// For checks: what terminal `id`'s view is told or shows.
    pub(super) fn term_probe(&self, id: u32, probe: TermProbe) -> Option<TermAnswer> {
        if let TermProbe::Settings(change) = &probe {
            let changed = crate::browser::settings::change(change).is_ok();
            let _ = self.proxy.send_event(Cmd::Restyle);
            return changed.then_some(TermAnswer::Done);
        }
        if let TermProbe::Window(width, height) = probe {
            let deck = self.deck.as_ref()?;
            let (was_wide, was_tall) = deck.sized(f64::from(width), f64::from(height));
            return Some(TermAnswer::Window(was_wide, was_tall));
        }
        #[cfg(target_os = "macos")]
        if let TermProbe::Press(pointing, x, y) = probe {
            use tao::platform::macos::WindowExtMacOS;
            let native = &self.deck.as_ref()?.frame.native;
            let at = (f64::from(x), f64::from(y));
            let pressed = crate::browser::term_view::TermView::press_in(native, pointing, at);
            // The window hears of a press as it hears of a person's (presses.rs), which a test's
            // window does not watch.
            if pressed && pointing == crate::browser::term_view::Pointing::Down {
                let window = native.ns_window().addr();
                let _ = self.proxy.send_event(Cmd::Pressed(window, at.0, at.1));
            }
            return pressed.then_some(TermAnswer::Done);
        }
        if matches!(probe, TermProbe::Edge(_) | TermProbe::EdgeTold) {
            let deck = self.deck.as_ref()?;
            return deck.edge_held(&probe).then_some(TermAnswer::Done);
        }
        if probe == TermProbe::Carry {
            return self
                .deck
                .as_ref()
                .map(|deck| TermAnswer::Carry(deck.carry_seen()));
        }
        #[cfg(target_os = "macos")]
        if let TermProbe::KeyIn(code, chars) = &probe {
            let native = &self.deck.as_ref()?.frame.native;
            let typed = crate::browser::term_view::TermView::key_in(native, *code, chars);
            return typed.then_some(TermAnswer::Done);
        }
        #[cfg(target_os = "macos")]
        if probe == TermProbe::Close {
            let native = &self.deck.as_ref()?.frame.native;
            let window = crate::browser::appkit::native(native);
            let none = std::ptr::null::<objc2::runtime::AnyObject>();
            // SAFETY: a window closes as its close button closes it, with a sender (nil here).
            let _: () = unsafe { objc2::msg_send![window, performClose: none] };
            return Some(TermAnswer::Done);
        }
        if probe == TermProbe::Views {
            return self
                .deck
                .as_ref()
                .map(|deck| TermAnswer::Views(deck.term_views()));
        }
        let view = self
            .deck
            .as_ref()
            .and_then(|open| open.term(id)?.face.term());
        view.and_then(|view| view.probe(probe))
    }

    /// A picture of terminal `id` as its view draws it, sent to `reply` once encoded.
    pub(super) fn term_shot(&self, id: u32, reply: SyncSender<Result<Shot, String>>) {
        let view = self
            .deck
            .as_ref()
            .and_then(|open| open.term(id)?.face.term());
        match view {
            Some(view) => view.shot(reply),
            None => drop(reply.send(Err(format!("terminal t{id} has no view in the window")))),
        }
    }

    /// Panel `id`'s newest frame, run in its page; a terminal's drawn in its view, and the count
    /// of a find open under it told again where the new output changed it.
    pub(super) fn paint(&mut self, id: u32) {
        let Some(open) = self.deck.as_ref().and_then(|open| open.term(id)) else {
            return;
        };
        let frames = open.slot.latest();
        match (&open.face, frames) {
            (panel::Face::Page(page), Some(frame)) => drop(page.evaluate_script(&frame)),
            (panel::Face::Term(view), _) => {
                view.draw();
                let told = view.told();
                self.program_told(id, told);
                if let Some(open) = self.deck.as_mut() {
                    open.recount(id);
                }
            }
            (panel::Face::Page(_), None) => {}
        }
    }

    /// What terminal `id`'s program asked of the window as it drew: its copy written to the
    /// pasteboard where the rule lets it (`panel::copy_where`), its bell news for the person
    /// where they were not looking at it (`panel::unattended`'s rule), nothing in sight.
    fn program_told(&self, id: u32, (copied, rang): (Option<String>, bool)) {
        let Some(deck) = self.deck.as_ref() else {
            return;
        };
        if let Some(text) = copied {
            panel::copy_where(deck, id, &text, platform::copy);
        }
        #[cfg(target_os = "macos")]
        let active = crate::browser::alerts_macos::active();
        #[cfg(not(target_os = "macos"))]
        let active = true;
        let looking = active && panel::Keys::key_window(deck) && panel::Keys::in_sight(deck, id);
        if rang && !looking {
            (self.shared).heard(&Key::Term(id).name(), Heard::Rang);
        }
    }
}
