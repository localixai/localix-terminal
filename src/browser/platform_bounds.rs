//! Where a view stands in its window, set only when it changes: every layout places every view
//! (the strip, the side, the toolbar, each tab), and most of them stand where they stood.

use tao::dpi::LogicalSize;

/// A view's place, in points from the window's top left: across, down, wide, tall.
type Spot = [f64; 4];

/// What a view was last given (`asked`), and where it stood just after (`stood`, as the system
/// reads it back: whole points, in its window's terms).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    asked: Spot,
    stood: Spot,
}

/// Whether a view that stands at `now` and is asked for `asked` stands there already: it was
/// given that place last (`last`), and nothing has moved it since (the window resized under it,
/// another view at its address).
fn stands(last: Option<&Placed>, asked: Spot, now: Spot) -> bool {
    last.is_some_and(|last| last.asked == asked && last.stood == now)
}

#[cfg(target_os = "macos")]
thread_local! {
    /// Each view's last place, by its address: the main thread's, as the views are. Kept small:
    /// past a few hundred (views gone), it starts over.
    static PLACED: std::sync::Mutex<std::collections::HashMap<usize, Placed>> =
        std::sync::Mutex::default();
}

#[cfg(target_os = "macos")]
fn spot(rect: wry::Rect) -> Spot {
    let (at, size) = (
        rect.position.to_logical::<f64>(1.0),
        rect.size.to_logical::<f64>(1.0),
    );
    [at.x, at.y, size.width, size.height]
}

/// `webview` `size` large, `x` across and `top` down its window, unless it stands there already.
#[cfg(target_os = "macos")]
pub(in super::super) fn bound(webview: &wry::WebView, size: LogicalSize<f64>, x: f64, top: f64) {
    use wry::WebViewExtMacOS;
    const KEPT: usize = 512;
    let asked = [x, top, size.width, size.height];
    // The native view's address: the same for as long as the view lives, wherever it moves.
    let view = objc2::rc::Retained::as_ptr(&webview.webview()).addr();
    let last = PLACED.with(|placed| crate::lock(placed).get(&view).copied());
    let Ok(now) = webview.bounds() else {
        let _ = webview.set_bounds(super::super::frame::fill(size, x, top));
        return;
    };
    if stands(last.as_ref(), asked, spot(now)) {
        return;
    }
    let _ = webview.set_bounds(super::super::frame::fill(size, x, top));
    if let Ok(stood) = webview.bounds() {
        PLACED.with(|placed| {
            let mut placed = crate::lock(placed);
            if placed.len() >= KEPT {
                placed.clear();
            }
            placed.insert(
                view,
                Placed {
                    asked,
                    stood: spot(stood),
                },
            );
        });
    }
}

/// Elsewhere every place is set: a GTK box lays its children out itself.
#[cfg(not(target_os = "macos"))]
pub(in super::super) fn bound(webview: &wry::WebView, size: LogicalSize<f64>, x: f64, top: f64) {
    let _ = webview.set_bounds(super::super::frame::fill(size, x, top));
}

#[cfg(test)]
#[path = "platform_bounds_tests.rs"]
mod tests;
