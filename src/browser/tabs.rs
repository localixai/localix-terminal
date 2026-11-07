//! A window's browser tabs (deck.rs): one WebView per tab, the active tab filling
//! the browser's part of the window while it is shown and the others beside it, out of sight but
//! still rendering (WebKit stops a hidden view's animation
//! frames; a tab moved aside is not hidden). A closed tab's WebView is kept, blank, and reused by
//! the next tab: on macOS the tabs share one WebKit configuration (cookies, the probe's IPC), and
//! the first WebView owns its IPC handler, so none is ever dropped before the window is.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tao::dpi::LogicalSize;
use tao::event_loop::EventLoopProxy;
use wry::WebView;

use super::host::{Cmd, Shared, Window};
use super::platform::keep_tab_rendering;
use super::popup::{self, Parent};
use super::sizes::{self, Frame};
use super::strip::Key;
use super::tab_view::{build, builder, share};

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

    /// Draw the page at `zoom`, when it is not at it already.
    fn zoom(&self, zoom: f64) {
        if (self.zoomed() - zoom).abs() > f64::EPSILON && self.webview.zoom(zoom).is_ok() {
            self.zoom.store(zoom.to_bits(), Ordering::Relaxed);
        }
    }

    fn zoomed(&self) -> f64 {
        f64::from_bits(self.zoom.load(Ordering::Relaxed))
    }
}

/// A page on its way to another window: its view as it is, moved and not loaded again
/// (docs/windows.md), with what its window kept of it.
pub(super) struct Carried {
    slot: Slot,
    frame: Option<Frame>,
    window: Window,
}

impl Carried {
    /// How its window opened its pages: one with none of its own yet opens them the same.
    pub(super) fn window(&self) -> &Window {
        &self.window
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
            let project = scope.take_opening();
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
                slot.open = true;
                // A tab used again starts its loads over: the last page's are not this one's.
                crate::lock(&shared.loads).remove(&slot.id);
                slot.webview
                    .load_url(url)
                    .map_err(|error| error.to_string())?;
                slot.id
            }
            None => {
                let id = popup::reserve().ok_or_else(full)?;
                let webview = self.build(native, id, url, shared, proxy)?;
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

    /// No page yet, in `native`: its pages come from other windows.
    pub(super) fn empty(native: &tao::window::Window, window: &Window) -> Self {
        // Joined, the pages end with `tabs` (its drop), as an opened one's do; the places stay.
        popup::join();
        Self {
            slots: vec![],
            active: 1,
            window: window.clone(),
            frames: HashMap::new(),
            native: native.id(),
        }
    }

    /// Page `id` leaves for another window, its view as it is: the page acted on here is the one
    /// before it, as when it closes.
    pub(super) fn take(&mut self, id: u32, shared: &Shared) -> Option<Carried> {
        let at = (self.slots.iter()).position(|slot| slot.id == id && slot.open)?;
        let slot = self.slots.remove(at);
        let frame = self.frames.remove(&id);
        let ids = self.ids();
        if self.active == id
            && let Some(last) = ids.last()
        {
            self.active = (ids.iter().rev().find(|other| **other < id)).map_or(*last, |one| *one);
        }
        shared.set_tabs(self.native, self.active, &ids);
        Some(Carried {
            slot,
            frame,
            window: self.window.clone(),
        })
    }

    /// A page come from another window: its view is put in `native`, this window, as it was,
    /// and it is the page acted on.
    pub(super) fn take_in(
        &mut self,
        carried: Carried,
        native: &tao::window::Window,
        shared: &Shared,
    ) {
        super::platform::reparent(&carried.slot.webview, native);
        let id = carried.slot.id;
        if let Some(frame) = carried.frame {
            self.frames.insert(id, frame);
        }
        self.slots.push(carried.slot);
        self.active = id;
        shared.set_tabs(self.native, self.active, &self.ids());
    }

    fn check(&self, id: u32) -> Result<(), String> {
        match self.slot(id) {
            Some(_) => Ok(()),
            None => Err(format!("there is no tab {id}: {}", list(&self.ids()))),
        }
    }

    /// The tabs open now, and the one acted on.
    pub(super) fn listed(&self) -> (Vec<u32>, u32) {
        (self.ids(), self.active)
    }

    /// Tab `id`'s zoom (the one acted on with none): points per CSS pixel, 1 unless its size is
    /// drawn smaller to fit the window. A picture's clip, in CSS pixels, is in points times it.
    pub(super) fn zoom(&self, id: Option<u32>) -> f64 {
        let id = id.unwrap_or(self.active);
        self.slot(id).map_or(1.0, Slot::zoomed)
    }

    /// Tab `id`'s size (the one acted on with none), while one is set.
    pub(super) fn frame(&self, id: Option<u32>) -> Option<&Frame> {
        self.frames.get(&id.unwrap_or(self.active))
    }

    /// Set tab `id`'s size (the one acted on with none), or let it fill the window again.
    pub(super) fn set_frame(&mut self, id: Option<u32>, frame: Option<Frame>) {
        let id = id.unwrap_or(self.active);
        match frame {
            Some(frame) => self.frames.insert(id, frame),
            None => self.frames.remove(&id),
        };
    }

    /// The tabs `shown` (one, or two side by side), each its id and its part of the window: its
    /// size, and `left` and `top` points from the window's corner. The others stand beside,
    /// out of sight.
    pub(super) fn layout(&self, shown: &[Pane]) {
        for slot in &self.slots {
            let pane = shown.iter().find(|pane| pane.0 == slot.id);
            let here = pane.is_some();
            let (_, size, left, top) =
                pane.copied()
                    .unwrap_or((0, LogicalSize::new(0.0, 0.0), 0.0, 0.0));
            fit(&slot.webview, None);
            let away = |x: f64| {
                if here {
                    left + x
                } else {
                    -(size.width + left + 10_000.0)
                }
            };
            if let Some(frame) = self.frames.get(&slot.id) {
                // A size of its own: in the middle, below the strip, nothing of it under it.
                // Larger than the window, drawn smaller: the page lays out at its size still.
                let placed = sizes::place(size, top, Some(frame));
                place(
                    &slot.webview,
                    away(placed.across),
                    placed.down,
                    placed.size,
                    here,
                );
                fit(&slot.webview, Some(placed.size));
                slot.zoom(placed.scale);
                continue;
            }
            slot.zoom(1.0);
            // Below the strip, never under it: the strip is the window's glass over every tab.
            place(&slot.webview, away(0.0), top, size, here);
        }
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
    format!("{MAX_TABS} tabs are open already: close one first")
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

/// Where a tab stands. A WebView in a GTK box has no bounds of its own, so there the tabs not
/// shown are hidden instead (WebKitGTK keeps running a hidden view's timers).
/// A page shown: its tab's id, its size, and where it starts.
pub(super) type Pane = (u32, LogicalSize<f64>, f64, f64);

#[cfg(not(target_os = "linux"))]
pub(super) fn place(webview: &WebView, x: f64, top: f64, size: LogicalSize<f64>, _shown: bool) {
    let _ = webview.set_bounds(super::frame::fill(size, x, top));
    super::platform::follow(webview, super::platform::Follow::Fill);
}

#[cfg(target_os = "linux")]
pub(super) fn place(webview: &WebView, _x: f64, _top: f64, _size: LogicalSize<f64>, shown: bool) {
    let _ = webview.set_visible(shown);
}

/// A page of a size of its own keeps it while the window is resized (the layout the resize
/// brings moves it to the middle again); with none it fills the window. In a GTK box: that size
/// asked for, in the middle of the page's share.
#[cfg(not(target_os = "linux"))]
fn fit(webview: &WebView, framed: Option<LogicalSize<f64>>) {
    if framed.is_some() {
        super::platform::follow(webview, super::platform::Follow::Centre);
    }
}

#[cfg(target_os = "linux")]
fn fit(webview: &WebView, framed: Option<LogicalSize<f64>>) {
    use gtk::prelude::WidgetExt;
    use wry::WebViewExtUnix;
    let view = webview.webview();
    let (size, align) = match framed {
        Some(size) => (size.cast::<i32>(), gtk::Align::Center),
        None => (LogicalSize::new(-1, -1), gtk::Align::Fill),
    };
    view.set_size_request(size.width, size.height);
    view.set_halign(align);
    view.set_valign(align);
}

/// The strip along the top, `size` tall. In a GTK box every page takes an equal share unless
/// told otherwise: the strip is held to its height there, and the pages under it fill the rest.
#[cfg(not(target_os = "linux"))]
pub(super) fn pin(strip: &WebView, size: LogicalSize<f64>) {
    let _ = strip.set_bounds(super::frame::fill(size, 0.0, 0.0));
    super::platform::follow(strip, super::platform::Follow::Top);
}

#[cfg(target_os = "linux")]
pub(super) fn pin(strip: &WebView, size: LogicalSize<f64>) {
    use gtk::prelude::{BoxExt, Cast, WidgetExt};
    use wry::WebViewExtUnix;
    let view = strip.webview();
    let tall = size.cast::<i32>().height;
    view.set_size_request(-1, tall);
    if let Some(parent) = view.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
        parent.set_child_packing(&view, false, false, 0, gtk::PackType::Start);
    }
}

impl Drop for Tabs {
    /// The window's pages are gone: the app's places start over once no window has any.
    fn drop(&mut self) {
        popup::end();
    }
}
