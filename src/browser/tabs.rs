//! The browser's tabs in the one window (deck.rs): one WebView per tab, the active tab filling
//! the browser's part of the window while it is shown and the others beside it, out of sight but
//! still rendering (WebKit stops a hidden view's animation
//! frames; a tab moved aside is not hidden). A closed tab's WebView is kept, blank, and reused by
//! the next tab: on macOS the tabs share one WebKit configuration (cookies, the probe's IPC), and
//! the first WebView owns its IPC handler, so none is ever dropped before the window is.

use std::collections::HashMap;
use std::sync::Arc;

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

/// One WebView: a tab while `open`, else blank and waiting to be reused. Its id is its place + 1.
struct Slot {
    webview: WebView,
    open: bool,
}

pub(super) struct Tabs {
    slots: Vec<Slot>,
    /// The id of the tab shown and acted on.
    active: u32,
    window: Window,
    /// Each tab's size, by id, while one is set (sizes.rs); else it fills the window.
    frames: HashMap<u32, Frame>,
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
        };
        popup::reset();
        let id = popup::reserve().unwrap_or(1);
        let webview = tabs.build(native, id, &window.url, shared, proxy)?;
        keep_tab_rendering(&webview);
        tabs.slots.push(Slot {
            webview,
            open: true,
        });
        shared.set_tabs(1, &tabs.ids());
        Ok(tabs)
    }

    /// The tab shown and acted on.
    pub(super) fn active(&self) -> &WebView {
        &self.slots[self.active as usize - 1].webview
    }

    /// Tab `id` while it is open, else none; with no id, the one acted on. A tab closed after its
    /// driver looked is never swapped for another.
    pub(super) fn at(&self, id: Option<u32>) -> Option<&WebView> {
        let Some(id) = id else {
            return Some(self.active());
        };
        let slot = self.slots.get((id as usize).wrapping_sub(1));
        slot.filter(|slot| slot.open).map(|slot| &slot.webview)
    }

    fn ids(&self) -> Vec<u32> {
        let open = self.slots.iter().enumerate().filter(|(_, slot)| slot.open);
        open.map(|(at, _)| id_of(at)).collect()
    }

    /// Open `url` in a new tab and show it; its id, or why there is none.
    pub(super) fn add(
        &mut self,
        native: &tao::window::Window,
        url: &str,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<u32, String> {
        // A popup built meanwhile takes the place it was given first.
        self.adopt(shared);
        let id = match self.slots.iter().position(|slot| !slot.open) {
            Some(at) => {
                popup::claim(id_of(at));
                let slot = &mut self.slots[at];
                slot.open = true;
                // A tab used again starts its loads over: the last page's are not this one's.
                crate::lock(&shared.loads).remove(&id_of(at));
                slot.webview
                    .load_url(url)
                    .map_err(|error| error.to_string())?;
                id_of(at)
            }
            None if self.slots.len() < MAX_TABS => {
                let id = popup::reserve().ok_or_else(full)?;
                let webview = self.build(native, id, url, shared, proxy)?;
                keep_tab_rendering(&webview);
                self.slots.push(Slot {
                    webview,
                    open: true,
                });
                id
            }
            None => return Err(full()),
        };
        // Of the project of the tab it was opened from, until its opener names its own.
        let mut groups = crate::lock(&shared.groups);
        let opener = groups.get(&Key::Web(self.active).name()).cloned();
        groups.insert(Key::Web(id).name(), opener.unwrap_or_default());
        drop(groups);
        self.active = id;
        shared.set_tabs(id, &self.ids());
        Ok(id)
    }

    /// Take in the popups pages opened (popup.rs), each shown as it comes and of its opener's
    /// project; the last one's id, if any came.
    pub(super) fn adopt(&mut self, shared: &Shared) -> Option<u32> {
        let mut last = None;
        for (id, webview) in popup::waiting() {
            keep_tab_rendering(&webview);
            let at = id as usize - 1;
            let end = self.slots.len();
            match self.slots.get_mut(at) {
                // A closed tab's place: its blank view goes, the popup's takes it.
                Some(slot) if !slot.open => {
                    *slot = Slot {
                        webview,
                        open: true,
                    };
                }
                Some(_) => continue,
                None if at == end => self.slots.push(Slot {
                    webview,
                    open: true,
                }),
                // Places are handed out in order: one past the end was never given.
                None => continue,
            }
            let mut groups = crate::lock(&shared.groups);
            let opener = groups.get(&Key::Web(self.active).name()).cloned();
            groups.insert(Key::Web(id).name(), opener.unwrap_or_default());
            drop(groups);
            self.active = id;
            last = Some(id);
        }
        if last.is_some() {
            shared.set_tabs(self.active, &self.ids());
        }
        last
    }

    /// Show tab `id`.
    pub(super) fn switch(&mut self, id: u32, shared: &Shared) -> Result<(), String> {
        self.check(id)?;
        self.active = id;
        shared.set_tabs(id, &self.ids());
        Ok(())
    }

    /// Close tab `id`, showing the one before it when it was shown. Returns whether tabs are left:
    /// with none, the window closes.
    pub(super) fn close(&mut self, id: u32, shared: &Shared) -> Result<bool, String> {
        self.check(id)?;
        let slot = &mut self.slots[id as usize - 1];
        slot.open = false;
        self.frames.remove(&id);
        popup::free(id);
        let _ = slot.webview.load_url(BLANK);
        let ids = self.ids();
        let Some(last) = ids.last() else {
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
        shared.set_tabs(self.active, &ids);
        Ok(true)
    }

    fn check(&self, id: u32) -> Result<(), String> {
        match self.slots.get((id as usize).wrapping_sub(1)) {
            Some(slot) if slot.open => Ok(()),
            _ => Err(format!("there is no tab {id}: {}", list(&self.ids()))),
        }
    }

    /// The tabs open now, and the one acted on.
    pub(super) fn listed(&self) -> (Vec<u32>, u32) {
        (self.ids(), self.active)
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

    /// The active tab fills `size`, `top` points down, while the browser is `shown`; the others
    /// (and all of them while a terminal is shown) stand beside it, out of sight.
    pub(super) fn layout(&self, size: LogicalSize<f64>, top: f64, shown: bool) {
        for (at, slot) in self.slots.iter().enumerate() {
            let here = shown && id_of(at) == self.active;
            fit(&slot.webview, None);
            let away = |x: f64| if here { x } else { -(size.width + 100.0) };
            if let Some(frame) = self.frames.get(&id_of(at)) {
                // A size of its own: in the middle, below the strip, nothing of it under it.
                let (across, down, framed) = sizes::place(size, top, Some(frame));
                super::platform::inset(&slot.webview, 0.0);
                place(&slot.webview, away(across), down, framed, here);
                fit(&slot.webview, Some(framed));
                continue;
            }
            // Under the strip where WebKit keeps its viewport below it, else below the strip.
            match super::platform::inset(&slot.webview, top) {
                true => {
                    let whole = LogicalSize::new(size.width, size.height + top);
                    place(&slot.webview, away(0.0), 0.0, whole, here);
                }
                false => place(&slot.webview, away(0.0), top, size, here),
            }
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

/// Where a tab stands. A WebView in a GTK box has no bounds of its own, so there the tabs not
/// shown are hidden instead (WebKitGTK keeps running a hidden view's timers).
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

/// The id of the tab in slot `at`: ids count from 1, and there are at most `MAX_TABS`.
fn id_of(at: usize) -> u32 {
    u32::try_from(at + 1).unwrap_or(u32::MAX)
}
