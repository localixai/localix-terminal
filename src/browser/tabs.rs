//! A window's browser tabs (deck.rs): one WebView per tab, the active tab filling
//! the browser's part of the window while it is shown and the others beside it, out of sight but
//! still rendering (WebKit stops a hidden view's animation
//! frames; a tab moved aside is not hidden). A closed tab's WebView is kept, blank, and reused by
//! the next tab: on macOS the tabs share one WebKit configuration (cookies, the probe's IPC), and
//! the first WebView owns its IPC handler, so none is ever dropped before the window is.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use tao::event_loop::EventLoopProxy;
use wry::WebView;

use super::host::{Cmd, Shared, Window};
use super::platform::keep_tab_rendering;
use super::popup::{self, Parent};
use super::sizes::Frame;
use super::strip::Key;
use super::tab_view::{build, builder, share};

#[path = "tabs_transfer.rs"]
mod transfer;
pub(super) use transfer::Carried;

#[path = "tabs_layout.rs"]
mod layout;
pub(super) use layout::{Pane, pin, place};

pub use super::told::MAX_TABS;
pub(super) const BLANK: &str = "about:blank";

/// One WebView: a tab while `open`, else blank and waiting to be reused. Its id is its place
/// among the app's pages (popup.rs), whichever window it is in.
struct Slot {
    id: u32,
    webview: WebView,
    open: bool,
    /// The page's zoom now, an `f64`'s bits: points per CSS pixel, below 1 while a size larger
    /// than the window is drawn smaller to fit it (sizes.rs).
    zoom: AtomicU64,
}

impl Slot {
    fn new(id: u32, webview: WebView) -> Self {
        Self {
            id,
            webview,
            open: true,
            zoom: AtomicU64::new(1.0_f64.to_bits()),
        }
    }
}

pub(super) struct Tabs {
    slots: Vec<Slot>,
    /// The id of the tab shown and acted on.
    active: u32,
    window: Window,
    /// Each tab's size, by id, while one is set (sizes.rs); else it fills the window.
    frames: HashMap<u32, Frame>,
    /// The native window its pages are in.
    native: tao::window::WindowId,
}

impl Tabs {
    /// The browser in `native`, with its first tab on `window.url`.
    pub(super) fn open(
        native: &tao::window::Window,
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut tabs = Self {
            slots: vec![],
            active: 1,
            window: window.clone(),
            frames: HashMap::new(),
            native: native.id(),
        };
        // The app's first pages start the places over, and the configuration every later page
        // shares; another window's take the next places.
        let first = popup::begin();
        if first {
            super::tab_view::forget();
        }
        // Begun, the pages end with `tabs` (its drop), opened or not.
        let id = match popup::reserve() {
            Some(id) => id,
            None if first => 1,
            None => return Err(full()),
        };
        let webview = tabs.build(native, id, &window.url, shared, proxy)?;
        if first {
            super::tab_view::remember(&webview);
        }
        keep_tab_rendering(&webview);
        tabs.slots.push(Slot::new(id, webview));
        // The first page is of the project the call opening it named, as every later one.
        let opening = {
            let mut scope = crate::lock(&shared.scope);
            let project = scope.take_opening().map(|(project, _)| project);
            if let Some(project) = &project {
                scope.touched(project);
            }
            project
        };
        if let Some(project) = opening {
            crate::lock(&shared.groups).insert(Key::Web(id).name(), project);
        }
        shared.set_tabs(tabs.native, 1, &tabs.ids());
        Ok(tabs)
    }

    /// The tab shown and acted on.
    pub(super) fn active(&self) -> &WebView {
        let slot = self
            .slot(self.active)
            .or_else(|| self.slots.iter().find(|slot| slot.open));
        // A browser always has a tab open: it goes when its last one closes.
        &slot.unwrap_or(&self.slots[0]).webview
    }

    /// Whether no page is left at all, open or kept blank: its last went to another window. A
    /// window keeps no such browser (`active` answers a page).
    pub(super) fn is_bare(&self) -> bool {
        self.slots.is_empty()
    }

    /// Tab `id`'s slot while it is open.
    fn slot(&self, id: u32) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.id == id && slot.open)
    }

    /// The id of the tab acted on.
    pub(super) fn active_id(&self) -> u32 {
        self.active
    }

    /// Tab `id` while it is open, else none; with no id, the one acted on. A tab closed after its
    /// driver looked is never swapped for another.
    pub(super) fn at(&self, id: Option<u32>) -> Option<&WebView> {
        let Some(id) = id else {
            return Some(self.active());
        };
        self.slot(id).map(|slot| &slot.webview)
    }

    fn ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = (self.slots.iter())
            .filter(|slot| slot.open)
            .map(|slot| slot.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Open `url` in a new tab of `project` (none: of the tab it was opened from), the one
    /// acted on when it is `shown` (or when there is none); its id, or why there is none.
    pub(super) fn add(
        &mut self,
        native: &tao::window::Window,
        url: &str,
        project: Option<String>,
        shown: bool,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<u32, String> {
        // A popup built meanwhile takes the place it was given first.
        self.adopt(shared);
        // A closed tab's view is used again while its place is still free; one a popup of
        // another window took meanwhile goes (its place is that popup's now).
        let mut reused = None;
        while let Some(at) = self.slots.iter().position(|slot| !slot.open) {
            if popup::claim(self.slots[at].id) {
                reused = Some(at);
                break;
            }
            self.slots.remove(at);
        }
        let id = match reused {
            Some(at) => {
                let slot = &mut self.slots[at];
                // A tab used again starts its loads over: the last page's are not this one's.
                crate::lock(&shared.loads).remove(&slot.id);
                if let Err(error) = slot.webview.load_url(url) {
                    // Not opened: the place it claimed goes back, and the view stays blank.
                    popup::free(slot.id);
                    return Err(error.to_string());
                }
                slot.open = true;
                slot.id
            }
            None => {
                let id = popup::reserve().ok_or_else(full)?;
                let webview = match self.build(native, id, url, shared, proxy) {
                    Ok(webview) => webview,
                    Err(error) => {
                        popup::release(id);
                        return Err(error);
                    }
                };
                keep_tab_rendering(&webview);
                self.slots.push(Slot::new(id, webview));
                id
            }
        };
        if let Some(webview) = self.at(Some(id)) {
            see_through_until_drawn(webview, id, shared);
        }
        let mut groups = crate::lock(&shared.groups);
        let opener = groups.get(&Key::Web(self.active).name()).cloned();
        let project = project.or(opener).unwrap_or_default();
        groups.insert(Key::Web(id).name(), project);
        drop(groups);
        let ids = self.ids();
        if shown || !ids.contains(&self.active) || self.active == id {
            self.active = id;
        }
        shared.set_tabs(self.native, self.active, &ids);
        Ok(id)
    }

    /// Take in the popups pages opened (popup.rs), each of its opener's project and shown as it
    /// comes when that project is in sight; the last one's id, and whether it shows, if any came.
    pub(super) fn adopt(&mut self, shared: &Shared) -> Option<(u32, bool)> {
        let mut last = None;
        for (id, webview, opener) in popup::waiting() {
            keep_tab_rendering(&webview);
            match self.slots.iter_mut().find(|slot| slot.id == id) {
                // A closed tab's place: its blank view goes, the popup's takes it.
                Some(slot) if !slot.open => *slot = Slot::new(id, webview),
                Some(_) => continue,
                None => self.slots.push(Slot::new(id, webview)),
            }
            let mut groups = crate::lock(&shared.groups);
            let project = groups
                .get(&Key::Web(opener).name())
                .cloned()
                .unwrap_or_default();
            groups.insert(Key::Web(id).name(), project.clone());
            drop(groups);
            let mut scope = crate::lock(&shared.scope);
            scope.touched(&project);
            let shown = project.is_empty() || scope.in_sight(&project);
            drop(scope);
            if shown {
                self.active = id;
            }
            last = Some((id, shown));
        }
        if last.is_some() {
            shared.set_tabs(self.native, self.active, &self.ids());
        }
        last
    }

    /// Show tab `id`.
    pub(super) fn switch(&mut self, id: u32, shared: &Shared) -> Result<(), String> {
        self.check(id)?;
        self.active = id;
        shared.set_tabs(self.native, id, &self.ids());
        Ok(())
    }

    /// Close tab `id`, showing the one before it when it was shown. Returns whether tabs are left:
    /// with none, the browser part goes, and the window stays.
    pub(super) fn close(&mut self, id: u32, shared: &Shared) -> Result<bool, String> {
        self.check(id)?;
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.id == id && slot.open)
        else {
            return Err(format!("there is no tab {id}"));
        };
        slot.open = false;
        // Its notices, its armed answer and its project are the page's: the next page of this id
        // has none of them.
        shared.forget(&[Key::Web(id)]);
        self.frames.remove(&id);
        popup::free(id);
        let _ = slot.webview.load_url(BLANK);
        let ids = self.ids();
        let Some(last) = ids.last() else {
            shared.set_tabs(self.native, self.active, &ids);
            return Ok(false);
        };
        if self.active == id {
            self.active = ids
                .iter()
                .rev()
                .find(|other| **other < id)
                .copied()
                .unwrap_or(*last);
        }
        shared.set_tabs(self.native, self.active, &ids);
        Ok(true)
    }

    fn check(&self, id: u32) -> Result<(), String> {
        match self.slot(id) {
            Some(_) => Ok(()),
            None => Err(format!("there is no tab {id}: {}", list(&self.ids()))),
        }
    }

    /// Whether tab `id` is open.
    pub(super) fn has(&self, id: u32) -> bool {
        self.slot(id).is_some()
    }

    /// The tabs open now, and the one acted on.
    pub(super) fn listed(&self) -> (Vec<u32>, u32) {
        (self.ids(), self.active)
    }

    fn build(
        &self,
        native: &tao::window::Window,
        id: u32,
        url: &str,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<WebView, String> {
        let first = self.slots.first().map(|slot| &slot.webview);
        let builder = builder(
            id,
            Some(url),
            &self.window,
            shared,
            proxy,
            Parent::of(native),
        );
        let builder = share(builder, first);
        build(builder, native).map_err(|error| error.to_string())
    }
}

fn full() -> String {
    format!(
        "{MAX_TABS} pages are open in the whole app (all projects, and the person's own): close one of yours, or ask the person to"
    )
}

/// `ids` as the agent reads them.
fn list(ids: &[u32]) -> String {
    let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
    format!("the tabs are {}", ids.join(", "))
}

/// A new tab drawn through until its page drew (its load finished, `drawn`): a blank view is
/// white, and would flash over a dark window before a dark page draws. Through it shows the
/// window, in the system's light or dark, as Safari shows its own until the page comes.
pub(super) fn see_through_until_drawn(webview: &WebView, id: u32, shared: &Shared) {
    super::platform::see_through(webview, true);
    crate::lock(&shared.undrawn).insert(id);
}

/// Tab `id`'s page drew: a tab drawn through until then draws its own ground again, so a page
/// that sets none is white, as everywhere.
pub(super) fn drawn(shared: &Shared, id: u32, proxy: &EventLoopProxy<Cmd>) {
    if crate::lock(&shared.undrawn).remove(&id) {
        let _ = proxy.send_event(Cmd::Reveal(id));
    }
}

impl Drop for Tabs {
    /// The window's pages are gone: the app's places start over once no window has any.
    fn drop(&mut self) {
        popup::end();
    }
}
