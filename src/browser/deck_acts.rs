//! What is done to the window's tabs by their keys, the person's clicks on the strip and the
//! agent's calls alike, and the tabs as both read them.

use wry::WebView;

use super::super::host::{CenterDid, Heard, NAV_PAGE, STRIP, Shared, TABS};
use super::super::strip::{Click, Key};
use super::{Deck, Menu, Shown};

impl Deck {
    /// Show or close the tab `click` names; why not, when it is not there.
    pub(crate) fn act(&mut self, click: Click, shared: &Shared) -> Result<(), String> {
        use Click::{Clear, Listed, Unmenu};
        let (key, closing) = match click {
            Click::Show(key) => (key, false),
            Click::Close(key) => (key, true),
            Click::History | Click::Events | Listed | Unmenu | Click::Event(..) | Clear => {
                self.menu_click(click, shared);
                return Ok(());
            }
            Click::Typing(open) => {
                // The suggestions need the window, which the strip has only where it lies over
                // the pages (macOS); in GTK's column it would take their place.
                self.typing = open && cfg!(target_os = "macos");
                return Ok(());
            }
            Click::Back | Click::Forward => {
                self.walk(click);
                return Ok(());
            }
            // The host opens its page: the tab it needs is built on the event loop.
            Click::Visit(_) | Click::Offer(_) => return Ok(()),
            Click::Drag => {
                let _ = self.frame.native.drag_window();
                return Ok(());
            }
            // The host's: the window's presence, and a tab built on the event loop.
            Click::Collapse
            | Click::Quit
            | Click::NewTab(..)
            | Click::Frame(..)
            | Click::Projects(..)
            | Click::More(..)
            | Click::Tabs(..)
            | Click::Menu(..)
            | Click::Key(_)
            | Click::Agent(_)
            | Click::Glass(_) => {
                return Ok(());
            }
            Click::Side => {
                self.slide_side(shared);
                return Ok(());
            }
            // The host's: laid out without a refresh while the edge is dragged.
            Click::SideWidth(..)
            | Click::SplitMove(..)
            | Click::SplitEven
            | Click::Move(..)
            | Click::DropOver(..)
            | Click::DropOff
            | Click::Drop(..) => return Ok(()),
            Click::Reload => {
                if let (Some(web), Shown::Web) = (&self.web, self.shown) {
                    let _ = web.active().reload();
                }
                return Ok(());
            }
            Click::Outside => {
                self.outside(shared);
                return Ok(());
            }
            Click::Zoom => {
                let native = &self.frame.native;
                native.set_maximized(!native.is_maximized());
                return Ok(());
            }
            Click::Send | Click::Comment => {
                self.comment_click(&click, shared);
                return Ok(());
            }
        };
        self.menu = None;
        // The agent shows a terminal the person closed: a tab again.
        if let (false, Key::Term(id)) = (closing, key) {
            self.unhide_term(id);
        }
        if !self.there(key) {
            let ids: Vec<String> = self.listing(shared).into_iter().map(|tab| tab.id).collect();
            return Err(format!(
                "there is no tab {}: the tabs are {}",
                key.name(),
                ids.join(", ")
            ));
        }
        if !closing {
            self.split_follows(key);
        }
        match (closing, key) {
            (false, Key::Web(id)) => {
                if let Some(web) = self.web.as_mut() {
                    web.switch(id, shared)?;
                }
                self.show(Shown::Web);
            }
            (false, Key::Term(id)) => self.show(Shown::Term(id)),
            // The last tab closed leaves the window: its start of a new tab shows.
            (true, Key::Term(id)) => self.hide_term(id),
            (true, Key::Web(id)) => {
                let left = self.web.as_mut().map(|web| web.close(id, shared));
                match left {
                    Some(Ok(false)) => self.end_web(&[id], shared),
                    Some(Err(why)) => return Err(why),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// A click on the menus: one opens or closes, or the Event Center's button is chosen.
    fn menu_click(&mut self, click: Click, shared: &Shared) {
        match click {
            Click::History => {
                self.toggle(Menu::History);
            }
            // Opened: all it shows is seen.
            Click::Events => {
                if self.toggle(Menu::Events) {
                    shared.heard("", Heard::Center(CenterDid::Opened));
                }
            }
            Click::Listed => {
                if self.menu != Some(Menu::Events) {
                    self.menu = Some(Menu::Events);
                    shared.heard("", Heard::Center(CenterDid::Opened));
                }
            }
            Click::Event(event, button) => {
                self.menu = None;
                let button = usize::from(button);
                shared.heard("", Heard::Center(CenterDid::Chose { event, button }));
            }
            // Cleared: the list stays open, empty.
            Click::Clear => shared.heard("", Heard::Center(CenterDid::Cleared)),
            Click::Unmenu => self.menu = None,
            // Not a menu's.
            Click::Show(_)
            | Click::Close(_)
            | Click::Back
            | Click::Forward
            | Click::Visit(_)
            | Click::Offer(_)
            | Click::SplitMove(..)
            | Click::SplitEven
            | Click::Move(..)
            | Click::DropOver(..)
            | Click::DropOff
            | Click::Drop(..)
            | Click::Comment
            | Click::Send
            | Click::Drag
            | Click::Zoom
            | Click::Collapse
            | Click::Quit
            | Click::Reload
            | Click::Outside
            | Click::Side
            | Click::SideWidth(..)
            | Click::NewTab(..)
            | Click::Frame(..)
            | Click::Projects(..)
            | Click::More(..)
            | Click::Tabs(..)
            | Click::Menu(..)
            | Click::Typing(_)
            | Click::Key(_)
            | Click::Agent(_)
            | Click::Glass(_) => {}
        }
    }

    /// The person's settings, taken by the window and its own pages (never the browser's).
    pub(crate) fn restyle(&mut self, settings: &super::super::settings::Settings) {
        let script = super::super::settings::look_script(settings);
        for page in [&self.strip, &self.side, &self.nav, &self.line]
            .into_iter()
            .flatten()
        {
            let _ = page.evaluate_script(&script);
        }
        for open in &self.terms {
            let _ = open.webview.evaluate_script(&script);
        }
        self.frame.restyle(settings.theme);
    }

    /// A shortcut's doing in the strip's page (`address`, `size`): what a click there would do,
    /// the keyboard given to the strip first.
    pub(crate) fn keyed_in_strip(&self, what: &str) {
        // The address field and the page's size are the browser's toolbar's, over a page only.
        let view = match what {
            "address" | "size" => self.nav.as_ref().filter(|_| self.shown == Shown::Web),
            _ => self.strip.as_ref(),
        };
        if let Some(view) = view {
            let _ = view.focus();
            let _ = view.evaluate_script(&format!("K({})", serde_json::json!(what)));
        }
    }

    /// The side's search takes the keyboard, the side opened first when it is shut.
    pub(crate) fn find_tab(&mut self, shared: &Shared) {
        if self.side.is_none() {
            return;
        }
        if !self.side_open {
            self.slide_side(shared);
        }
        if let Some(side) = &self.side {
            let _ = side.focus();
            let _ = side.evaluate_script("K(\"find\")");
        }
    }

    /// Menu `menu` opens, or closes when it is the one open; whether it is open now.
    fn toggle(&mut self, menu: Menu) -> bool {
        self.menu = (self.menu != Some(menu)).then_some(menu);
        self.menu.is_some()
    }

    /// Back or forward in the page shown, when a page is.
    fn walk(&mut self, click: Click) {
        self.menu = None;
        if let (Some(web), Shown::Web) = (&self.web, self.shown) {
            let script = if click == Click::Back {
                "history.back()"
            } else {
                "history.forward()"
            };
            let _ = web.active().evaluate_script(script);
        }
    }

    /// The comment button turns commenting on or off; Send hands the comments held to the agent
    /// and ends it. Either way the turn follows: the agent's actions wait while the person comments.
    fn comment_click(&mut self, click: &Click, shared: &Shared) {
        if *click == Click::Send {
            for (tab, said) in shared.comments.take() {
                shared.heard(&tab, Heard::Sent(&said));
            }
            self.sent = self.sent.wrapping_add(1);
            // Sent: the agent's turn again, to act on what was said.
            self.commenting = false;
        } else {
            self.commenting = !self.commenting;
            self.menu = None;
        }
        shared.turn.commenting(self.commenting);
    }

    /// This window's browser is gone, its pages `ids` with it: what the app keeps of them goes
    /// (no other window's does), and the person is no longer commenting on a page of it.
    pub(crate) fn end_web(&mut self, ids: &[u32], shared: &Shared) {
        shared.closed(self.frame.native.id(), ids);
        self.remove_web();
        self.stop_commenting(shared);
    }

    /// Whether the person comments on this window's pages now.
    pub(crate) fn is_commenting(&self) -> bool {
        self.commenting
    }

    /// The window went out of the person's sight: commenting ends, the pages and the strip say so.
    pub(crate) fn stop_commenting(&mut self, shared: &Shared) {
        if self.commenting {
            self.commenting = false;
            shared.turn.commenting(false);
            self.refresh(shared);
        }
    }

    /// Panel `id`'s page, or the strip's (`STRIP`).
    pub(crate) fn page(&self, id: u32) -> Option<&WebView> {
        match id {
            STRIP => self.strip.as_ref(),
            TABS => self.side.as_ref().or(self.strip.as_ref()),
            NAV_PAGE => self.nav.as_ref(),
            id => self.term(id).map(|open| &open.webview),
        }
    }

    /// The glass under the strip's open list at `rect` of the strip, or none.
    pub(crate) fn glass(&self, rect: Option<(u32, u32, u32, u32)>) {
        // Under the list that is open: the address field's suggestions are the browser's
        // toolbar's, the menus the strip's.
        let over = if self.typing { &self.nav } else { &self.strip };
        if let (Some(pane), Some(view)) = (&self.pane, over) {
            pane.place(view, rect);
        }
    }

    /// The page shown opened in the person's own browser, when it is at an address one opens.
    fn outside(&self, shared: &Shared) {
        let (Some(web), Shown::Web) = (&self.web, self.shown) else {
            return;
        };
        let active = web.active_id();
        let tabs = crate::lock(&shared.tabs);
        let url = tabs.iter().find(|tab| tab.id == active).map(|tab| &tab.url);
        if let Some(url) = url.filter(|url| super::super::guard::outside(url)) {
            super::super::platform::open_outside(url);
        }
    }

    /// The person picks one of `items` from the system's menu at `at` (the strip's points).
    /// The item `marked`, if any, is shown as the one chosen now.
    pub(crate) fn choose(
        &self,
        at: (f64, f64),
        items: &[&str],
        marked: Option<usize>,
    ) -> Option<usize> {
        let strip = self.strip.as_ref()?;
        super::super::platform::choose(strip, at, items, marked)
    }

    /// The person picks from `entries` (with lines, submenus, pictures) in the system's menu at
    /// `at` (the strip's points): the place of the item picked among them.
    pub(crate) fn pick(
        &self,
        at: (f64, f64),
        entries: &[super::super::choice::Entry],
    ) -> Option<usize> {
        let strip = self.strip.as_ref()?;
        super::super::platform::show(strip, at, entries)
    }

    /// Split, the tab picked (`key`) takes the half in use, or changes places with the one beside.
    fn split_follows(&mut self, key: Key) {
        let was = self.shown_key();
        self.split.beside = super::super::split::after_show(self.split.beside, was, key);
    }
}
