//! Construction of a deck and its four trusted strip views, before tabs are opened.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use wry::{WebView, WebViewBuilder};

use super::super::frame::{self, Frame, Look};
use super::super::host::{Cmd, Shared, Window};
use super::super::panel::Opening;
use super::super::strip::{self, HEIGHT};
use super::super::tab_view::{build_own, own_page};
use super::super::viewport::Viewport;
use super::super::{placement, platform};
use super::{Deck, LOADS, Part, Shown};

impl Deck {
    /// The window for the browser, on its first tab.
    pub(in super::super) fn for_web(
        (target, steps): (&EventLoopWindowTarget<Cmd>, usize),
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            (target, steps),
            &window.title,
            &window.viewport,
            window.headless,
            (shared, proxy),
        )?;
        deck.open_web(window, shared, proxy)?;
        Ok(deck)
    }

    /// A window with nothing in it yet, `steps` after the main one (0: the main one): the app
    /// opened with nothing left open, or a tab carried out of a list comes into it
    /// (docs/windows.md).
    pub(in super::super) fn empty(
        (target, steps): (&EventLoopWindowTarget<Cmd>, usize),
        shared: &Shared,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        Self::build(
            (target, steps),
            "llxterm",
            &Viewport::DESKTOP,
            false,
            (shared, proxy),
        )
    }

    /// The window for a terminal session's screen, before the browser opened.
    pub(in super::super) fn for_term(
        (target, steps): (&EventLoopWindowTarget<Cmd>, usize),
        opening: Opening,
        shared: &Shared,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            (target, steps),
            "llxterm",
            &Viewport::DESKTOP,
            false,
            (shared, proxy),
        )?;
        deck.add_term(opening, true, proxy)?;
        Ok(deck)
    }

    /// The window with nothing in it yet but its strip (when shown), its content `viewport`.
    /// A window `steps` windows after the main one (0: the main one) opens cascaded that many
    /// steps from where the main one is kept.
    fn build(
        (target, steps): (&EventLoopWindowTarget<Cmd>, usize),
        title: &str,
        viewport: &Viewport,
        headless: bool,
        (shared, proxy): (&Shared, &EventLoopProxy<Cmd>),
    ) -> Result<Self, String> {
        let look = frame::look(headless, frame::transparency());
        let top = if headless { 0.0 } else { HEIGHT };
        // A shown window opens in the middle of the person's screen, or where they left it; a
        // hidden one keeps the page's own size.
        let areas = match headless {
            true => Vec::new(),
            false => platform::work_areas(target),
        };
        let placed = (!areas.is_empty()).then(|| {
            let deck = placement::deck(&areas, crate::lock(&shared.kept).deck);
            match steps {
                0 => deck,
                steps => placement::cascade(placement::home(&areas, deck), deck, steps),
            }
        });
        let (size, at) = match placed {
            Some(at) => (
                LogicalSize::new(at.width, at.height),
                Some(LogicalPosition::new(at.x, at.y)),
            ),
            None => (
                LogicalSize::new(viewport.width, viewport.height + top),
                None,
            ),
        };
        // Coated in the person's theme: the titlebar and the strip are not the bare material's
        // grey. The browser's pages paint their own ground over it, opaque.
        let mut frame = Frame::build(target, (title, size, at), look)?;
        let strip = match headless {
            true => None,
            false => {
                let strip = strip_page(&frame.native, look, proxy.clone(), Part::Strip)?;
                // Only the main window is put away by its minimize button (it keeps drawing out
                // of sight): another window's minimize button is the system's.
                if steps == 0 {
                    platform::collapse_button(&frame.native, proxy);
                }
                // Shows the window too, without making it the key window.
                frame.show(&strip);
                Some(strip)
            }
        };
        // The tabs' column, where the pages lie side by side with it (not in GTK's column).
        let side = match (&strip, cfg!(target_os = "linux")) {
            (Some(_), false) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Side)?),
            _ => None,
        };
        let nav = match &strip {
            Some(_) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Nav)?),
            None => None,
        };
        // Split only where the pages lie side by side with the side (not in GTK's column).
        let line = match &side {
            Some(_) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Line)?),
            None => None,
        };
        let docs = match &strip {
            Some(_) => Some(strip_page(&frame.native, look, proxy.clone(), Part::Docs)?),
            None => None,
        };
        let (side_open, side_width) = {
            let kept = crate::lock(&shared.kept);
            (
                kept.side.unwrap_or(true),
                (kept.side_width).map_or(strip::SIDE, |width| strip::side_held(width, None)),
            )
        };
        let pane = strip.as_ref().and_then(platform::Pane::under);
        Ok(Self {
            frame,
            look,
            strip,
            side,
            nav,
            line,
            docs,
            stacks: std::sync::Mutex::default(),
            split: super::super::split::Split::default(),
            drop: None,
            landing: None,
            side_open,
            side_width,
            side_now: if side_open { side_width } else { strip::RAIL },
            pane,
            said: std::sync::Mutex::default(),
            held: super::said::Held::default(),
            web: None,
            terms: vec![],
            shown: Shown::Web,
            menu: None,
            typing: false,
            offer: std::sync::Mutex::default(),
            commenting: false,
            sent: 0,
            proxy: proxy.clone(),
        })
    }
}

fn strip_page(
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
    part: Part,
) -> Result<WebView, String> {
    // Every part draws the agents' marks (the side's tabs, the bar's crumbs and offer).
    let marks = super::super::agents::marks();
    let page = frame::dress(&strip::page(&marks), look);
    let page = match part {
        Part::Strip => page,
        Part::Side => page.replacen("<html", "<html data-part=\"side\"", 1),
        Part::Nav => page.replacen("<html", "<html data-part=\"nav\"", 1),
        Part::Line => page.replacen("<html", "<html data-part=\"line\"", 1),
        Part::Docs => page.replacen("<html", "<html data-part=\"docs\"", 1),
    };
    let loaded = proxy.clone();
    let builder = own_page(WebViewBuilder::new())
        // The first click in a window that is not the key one reaches the page: the person
        // never clicks twice, once to wake the window and once to do what they meant.
        .with_accept_first_mouse(true)
        .with_html(page)
        // The person's fonts, before the page's own scripts (settings.rs).
        .with_initialization_script(super::super::settings::look_script(
            &super::super::settings::now(),
        ))
        .with_transparent(look == Look::Glass)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(posted(part, request.body().clone()));
        })
        // Its tabs are drawn once its page can draw them, all it was told told again.
        .with_on_page_load_handler(move |_, _| {
            LOADS.fetch_add(1, Ordering::SeqCst);
            let _ = loaded.send_event(Cmd::Strip);
        });
    build_own(builder, native).map_err(|error| error.to_string())
}

/// What a page of `part` posted (`said`), as the window hears it. A point it says is of its own
/// view, so each part that lies elsewhere than the strip says whose points they are, and the
/// window moves them to where that part lies (host_dispatch.rs). The line between the halves
/// says no point of its own.
fn posted(part: Part, said: String) -> Cmd {
    match part {
        Part::Strip | Part::Line => Cmd::Clicked(said),
        Part::Side => Cmd::SideClicked(said),
        Part::Nav => Cmd::NavClicked(said),
        Part::Docs => Cmd::DocsClicked(said),
    }
}

#[cfg(test)]
#[path = "deck_build_tests.rs"]
mod tests;
