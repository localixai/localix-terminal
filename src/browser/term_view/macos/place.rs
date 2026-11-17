//! The terminal's view placed in its window: made in the window's content view, out of sight
//! until it is placed where the tab's pane is (following the window as it resizes), given the
//! keys, asked whether it has them, moved into another window with its terminal going on, and
//! taken out of its window as its tab lets it go (`Owned`).

use std::ops::Deref;
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use tao::dpi::LogicalSize;

use super::view::{State, TermView};
use super::{access, events, ns, pointer};
use crate::surface::Surface;

impl TermView {
    /// A view of `surface` in `native`, out of sight until it is placed, which opens the links
    /// the person Command-clicks with `opens`; none off the main thread or when no font of the
    /// terminal's can be made.
    pub(crate) fn open(
        native: &tao::window::Window,
        surface: Arc<dyn Surface>,
        opens: Box<dyn Fn(&str)>,
    ) -> Option<Owned> {
        use tao::platform::macos::WindowExtMacOS;
        let mtm = MainThreadMarker::new()?;
        let state = State::new(surface, native.scale_factor(), opens);
        let root = Retained::clone(&state.layers.root);
        let this = Self::alloc(mtm).set_ivars(Mutex::new(state));
        let zero = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 0.0));
        // SAFETY: NSView's designated initializer takes its frame.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: zero] };
        // The view hosts the terminal's own layer: it is set before the view wants one.
        // SAFETY: an NSView takes its layer, a CALayer.
        let _: () = unsafe { msg_send![&*view, setLayer: &*root] };
        // SAFETY: and whether it is layer-backed, a BOOL.
        let _: () = unsafe { msg_send![&*view, setWantsLayer: Bool::YES] };
        // SAFETY: and whether it is out of sight, a BOOL.
        let _: () = unsafe { msg_send![&*view, setHidden: Bool::YES] };
        // Nothing it draws reaches past its pane: a row of a wider grid, kept while the pane's
        // edge moves, is cut at it (AppKit masks the view's layer to its bounds so).
        // SAFETY: an NSView takes whether it clips what it draws to its bounds, a BOOL.
        let _: () = unsafe { msg_send![&*view, setClipsToBounds: Bool::YES] };
        pointer::track(&view);
        let content: *mut AnyObject = native.ns_view().cast();
        // SAFETY: the window's content view, alive with the window.
        if let Some(content) = unsafe { content.as_ref() } {
            // SAFETY: it adds the view on top (the strip and its lists are put over it again
            // as the window lays out).
            let _: () = unsafe { msg_send![content, addSubview: &*view] };
        }
        view.restyle();
        Some(Owned(view))
    }

    /// The view `size` large, `x` across and `top` down its window, following it as it
    /// resizes as a page of the window does (`platform::filling`); out of sight, it draws
    /// nothing and lets go of its pictures. The window's layout is the one place its grid
    /// follows its size from: the size the system gives it as the window's edge moves, before
    /// that layout, is a step between two, and its program is told nothing of it.
    pub(crate) fn place(&self, x: f64, top: f64, size: LogicalSize<f64>, shown: bool) {
        if let Some(parent) = ns::superview(self) {
            let bounds = ns::bounds(&parent);
            // SAFETY: an NSView answers whether it counts from the top, a BOOL.
            let flipped: bool = unsafe { msg_send![&*parent, isFlipped] };
            let y = if flipped {
                top
            } else {
                bounds.size.height - top - size.height
            };
            let frame = CGRect::new(CGPoint::new(x, y), CGSize::new(size.width, size.height));
            // SAFETY: an NSView takes its frame, a CGRect.
            let _: () = unsafe { msg_send![self, setFrame: frame] };
            let mask = crate::browser::platform::filling(self);
            // SAFETY: and its autoresizing mask, an NSUInteger.
            let _: () = unsafe { msg_send![self, setAutoresizingMask: mask] };
        }
        self.show(shown);
        self.fit();
        self.lifted();
    }

    /// The keys to this view, in its window.
    pub(crate) fn focus(&self) {
        if let Some(window) = ns::window(self) {
            // SAFETY: a window makes a responder its first.
            let _: bool = unsafe { msg_send![&*window, makeFirstResponder: self] };
        }
    }

    /// Whether the view holds its window's keys.
    pub(crate) fn has_keys(&self) -> bool {
        let Some(window) = ns::window(self) else {
            return false;
        };
        // SAFETY: a window answers its first responder, or nil.
        let first: *const AnyObject = unsafe { msg_send![&*window, firstResponder] };
        std::ptr::eq(first, std::ptr::from_ref(self).cast())
    }

    /// The view moved into `native`, as it is: its terminal goes on.
    pub(crate) fn reparent(&self, native: &tao::window::Window) {
        use tao::platform::macos::WindowExtMacOS;
        let content: *mut AnyObject = native.ns_view().cast();
        // SAFETY: the view leaves its superview.
        let _: () = unsafe { msg_send![self, removeFromSuperview] };
        // SAFETY: the window's content view, alive with the window.
        if let Some(content) = unsafe { content.as_ref() } {
            // SAFETY: it adds the view.
            let _: () = unsafe { msg_send![content, addSubview: self] };
        }
    }

    /// The view out of its window for good, at once: nothing outside calls it again (a
    /// selection's display link, its tracking, the window's key notices), the system's secure
    /// input is given back, VoiceOver forgets it, its pictures and what the terminal drew from
    /// are let go of, and its window's hierarchy, which held it, lets it go.
    fn close(&self) {
        pointer::untrack(self);
        events::secure(self, false);
        access::gone(self);
        if let Some(mut state) = self.state() {
            state.layers.clear();
            if let Some(painter) = state.painter.as_mut() {
                painter.forget();
            }
            state.surface.hidden();
        }
        // SAFETY: the view leaves its superview, which releases it.
        let _: () = unsafe { msg_send![self, removeFromSuperview] };
    }
}

/// The view as its tab holds it: let go of, it leaves its window with everything it holds
/// (`TermView::close`), and is freed as the last hold on it goes.
pub(crate) struct Owned(Retained<TermView>);

impl Deref for Owned {
    type Target = TermView;

    fn deref(&self) -> &TermView {
        &self.0
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.0.close();
    }
}
