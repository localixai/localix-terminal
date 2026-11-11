//! The pages under a dialog keep still: while the strip's dialog covers the window, the views
//! under it no longer follow the pointer, so none of them lights a row or sets its own cursor
//! through the veil. The system tells a view of the pointer whatever lies over it; the view's
//! tracking areas are taken off for the while (macOS) and put back as they were.

#[cfg(target_os = "macos")]
mod native {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use wry::{WebView, WebViewExtMacOS};

    /// A view kept still: itself (held, so its address is no other view's meanwhile) and the
    /// tracking areas taken off it.
    type Still = (Retained<AnyObject>, Vec<Retained<AnyObject>>);

    thread_local! {
        /// The views kept still, by their address: the main thread's, as the views are.
        static STILL: Mutex<HashMap<usize, Still>> = Mutex::default();
    }

    pub(in super::super) fn set(view: &WebView, on: bool) {
        let native: Retained<AnyObject> = Retained::into_super(Retained::into_super(
            Retained::into_super(Retained::into_super(Retained::into_super(view.webview()))),
        ));
        let at = Retained::as_ptr(&native).addr();
        STILL.with(|still| {
            let mut still = crate::lock(still);
            match (on, still.contains_key(&at)) {
                (true, false) => {
                    // SAFETY: an NSView answers its tracking areas, an NSArray.
                    let areas: Retained<AnyObject> = unsafe { msg_send![&*native, trackingAreas] };
                    // SAFETY: an NSArray answers how many it holds, an NSUInteger.
                    let count: usize = unsafe { msg_send![&*areas, count] };
                    let taken: Vec<Retained<AnyObject>> = (0..count)
                        // SAFETY: an NSArray answers the object at an index below its count.
                        .map(|index| unsafe { msg_send![&*areas, objectAtIndex: index] })
                        .collect();
                    for area in &taken {
                        // SAFETY: an NSView lets go of a tracking area it holds.
                        let _: () = unsafe { msg_send![&*native, removeTrackingArea: &**area] };
                    }
                    still.insert(at, (native, taken));
                }
                (false, true) => {
                    let Some((native, taken)) = still.remove(&at) else {
                        return;
                    };
                    for area in &taken {
                        // SAFETY: an NSView takes back a tracking area made for it.
                        let _: () = unsafe { msg_send![&*native, addTrackingArea: &**area] };
                    }
                }
                (true, true) | (false, false) => {}
            }
        });
    }
}

/// `view` kept still (`on`), or following the pointer again.
#[cfg(target_os = "macos")]
pub(super) use native::set;

/// `view` kept still (`on`), or following the pointer again: elsewhere a covered view is not
/// told of the pointer.
#[cfg(not(target_os = "macos"))]
pub(super) fn set(_view: &wry::WebView, _on: bool) {}
