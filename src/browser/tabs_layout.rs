//! Tab sizing, zoom and native placement, also used by the strip and terminal panels.

use std::sync::atomic::Ordering;

use tao::dpi::LogicalSize;
use wry::WebView;

use super::super::sizes::{self, Frame};
use super::{Slot, Tabs};

impl Slot {
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

impl Tabs {
    /// Tab `id`'s zoom (the one acted on with none): points per CSS pixel, 1 unless its size is
    /// drawn smaller to fit the window. A picture's clip, in CSS pixels, is in points times it.
    pub(in super::super) fn zoom(&self, id: Option<u32>) -> f64 {
        let id = id.unwrap_or(self.active);
        self.slot(id).map_or(1.0, Slot::zoomed)
    }

    /// Tab `id`'s size (the one acted on with none), while one is set.
    pub(in super::super) fn frame(&self, id: Option<u32>) -> Option<&Frame> {
        self.frames.get(&id.unwrap_or(self.active))
    }

    /// Set tab `id`'s size (the one acted on with none), or let it fill the window again.
    pub(in super::super) fn set_frame(&mut self, id: Option<u32>, frame: Option<Frame>) {
        let id = id.unwrap_or(self.active);
        match frame {
            Some(frame) => self.frames.insert(id, frame),
            None => self.frames.remove(&id),
        };
    }

    /// The tabs `shown` (one, or two side by side), each its id and its part of the window: its
    /// size, and `left` and `top` points from the window's corner. The others stand beside,
    /// out of sight.
    pub(in super::super) fn layout(&self, shown: &[Pane]) {
        for slot in &self.slots {
            let pane = shown.iter().find(|pane| pane.0 == slot.id);
            let here = pane.is_some();
            let (_, size, left, top) =
                pane.copied()
                    .unwrap_or((0, LogicalSize::new(0.0, 0.0), 0.0, 0.0));
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
                put(
                    &slot.webview,
                    (away(placed.across), placed.down),
                    placed.size,
                    here,
                );
                fit(&slot.webview, Some(placed.size));
                slot.zoom(placed.scale);
                continue;
            }
            slot.zoom(1.0);
            fit(&slot.webview, None);
            // Below the strip, never under it: the strip is the window's glass over every tab.
            place(&slot.webview, away(0.0), top, size, here);
        }
    }
}

/// A page shown: its tab's id, its size, and where it starts.
pub(in super::super) type Pane = (u32, LogicalSize<f64>, f64, f64);

/// Where a tab stands. A WebView in a GTK box has no bounds of its own, so there the tabs not
/// shown are hidden instead (WebKitGTK keeps running a hidden view's timers).
#[cfg(not(target_os = "linux"))]
pub(in super::super) fn place(
    webview: &WebView,
    x: f64,
    top: f64,
    size: LogicalSize<f64>,
    _shown: bool,
) {
    super::super::platform::bound(webview, size, x, top);
    super::super::platform::follow(webview, super::super::platform::Follow::Fill);
}

/// Where a page of a size of its own stands: placed only, the way it follows the window is
/// `fit`'s (set once, not as one that fills first and then as one in the middle).
#[cfg(not(target_os = "linux"))]
fn put(webview: &WebView, (x, top): (f64, f64), size: LogicalSize<f64>, _shown: bool) {
    super::super::platform::bound(webview, size, x, top);
}

#[cfg(target_os = "linux")]
fn put(webview: &WebView, (x, top): (f64, f64), size: LogicalSize<f64>, shown: bool) {
    place(webview, x, top, size, shown);
}

#[cfg(target_os = "linux")]
pub(in super::super) fn place(
    webview: &WebView,
    _x: f64,
    _top: f64,
    _size: LogicalSize<f64>,
    shown: bool,
) {
    let _ = webview.set_visible(shown);
}

/// A page of a size of its own keeps it while the window is resized (the layout the resize
/// brings moves it to the middle again); with none it fills the window. In a GTK box: that size
/// asked for, in the middle of the page's share.
#[cfg(not(target_os = "linux"))]
fn fit(webview: &WebView, framed: Option<LogicalSize<f64>>) {
    if framed.is_some() {
        super::super::platform::follow(webview, super::super::platform::Follow::Centre);
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
pub(in super::super) fn pin(strip: &WebView, size: LogicalSize<f64>) {
    super::super::platform::bound(strip, size, 0.0, 0.0);
    super::super::platform::follow(strip, super::super::platform::Follow::Top);
}

#[cfg(target_os = "linux")]
pub(in super::super) fn pin(strip: &WebView, size: LogicalSize<f64>) {
    use gtk::prelude::{BoxExt, Cast, WidgetExt};
    use wry::WebViewExtUnix;
    let view = strip.webview();
    let tall = size.cast::<i32>().height;
    view.set_size_request(-1, tall);
    if let Some(parent) = view.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
        parent.set_child_packing(&view, false, false, 0, gtk::PackType::Start);
    }
}
