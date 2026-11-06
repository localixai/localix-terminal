//! The glass under the strip's open list (macOS): the address field's suggestions and the
//! history menu are drawn by the strip's page, over the page shown, on the system's menu
//! material, rounded as a menu is. The page says where its list is (`glass x y w h`); the pane
//! follows it, hidden while none is open.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use wry::{WebView, WebViewExtMacOS};

use super::capture::{CGPoint, CGRect, CGSize};

/// A view of AppKit's `material`, blurring what of the window is behind it.
pub(super) fn material(material: isize) -> Retained<AnyObject> {
    /// NSVisualEffectBlendingModeWithinWindow, NSVisualEffectStateActive.
    const WITHIN: isize = 1;
    const ACTIVE: isize = 1;
    // SAFETY: `new` on NSVisualEffectView answers a retained view, laid out by its owner.
    let view: Retained<AnyObject> = unsafe { msg_send![class!(NSVisualEffectView), new] };
    // SAFETY: an NSVisualEffectView takes NSInteger modes.
    let _: () = unsafe { msg_send![&*view, setBlendingMode: WITHIN] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*view, setState: ACTIVE] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*view, setMaterial: material] };
    view
}

/// Put `view` just below the strip, among its siblings: over the pages, under the strip's page.
pub(super) fn below(view: &AnyObject, strip: &WebView) -> Option<()> {
    let strip = strip.webview();
    // SAFETY: an NSView answers its superview, or nil.
    let parent: Option<Retained<AnyObject>> = unsafe { msg_send![&*strip, superview] };
    let parent = parent?;
    // SAFETY: an NSView adds (or moves) a subview below a sibling (NSWindowBelow, -1).
    let _: () =
        unsafe { msg_send![&*parent, addSubview: view, positioned: -1_isize, relativeTo: &*strip] };
    Some(())
}

/// The strip over every page, a page built since included: its open list lies over the page
/// shown, which stays where it is.
pub(super) fn raise(strip: &WebView) -> bool {
    let view = strip.webview();
    // SAFETY: an NSView answers its superview, or nil.
    let parent: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, superview] };
    let Some(parent) = parent else {
        return false;
    };
    // SAFETY: an NSView answers its subviews, an NSArray.
    let subviews: Retained<AnyObject> = unsafe { msg_send![&*parent, subviews] };
    // SAFETY: an NSArray answers its last object, or nil.
    let last: Option<Retained<AnyObject>> = unsafe { msg_send![&*subviews, lastObject] };
    let strip_view: &AnyObject = &view;
    if !last.is_some_and(|last| std::ptr::eq(&*last, strip_view)) {
        // A sibling added again goes on top (NSWindowAbove, 1, relative to none).
        // SAFETY: an NSView adds (here: moves) a subview to the top of its siblings.
        let _: () = unsafe {
            msg_send![&*parent, addSubview: &*view, positioned: 1_isize, relativeTo: std::ptr::null::<AnyObject>()]
        };
    }
    true
}

/// The glass under the strip's open list.
pub(super) struct Pane(Retained<AnyObject>);

impl Pane {
    /// A pane under `strip`, hidden.
    pub(super) fn under(strip: &WebView) -> Option<Self> {
        /// NSVisualEffectMaterialMenu, and a menu's corners.
        const MENU: isize = 5;
        const ROUND: f64 = 10.0;
        let pane = material(MENU);
        // SAFETY: an NSView takes a BOOL.
        let _: () = unsafe { msg_send![&*pane, setHidden: Bool::YES] };
        // SAFETY: as above: drawn into a layer, which rounds its corners.
        let _: () = unsafe { msg_send![&*pane, setWantsLayer: Bool::YES] };
        // SAFETY: a view that wants a layer answers it.
        let layer: Option<Retained<AnyObject>> = unsafe { msg_send![&*pane, layer] };
        if let Some(layer) = layer {
            // SAFETY: a CALayer takes a CGFloat radius.
            let _: () = unsafe { msg_send![&*layer, setCornerRadius: ROUND] };
            // SAFETY: a CALayer takes a BOOL.
            let _: () = unsafe { msg_send![&*layer, setMasksToBounds: Bool::YES] };
        }
        below(&pane, strip)?;
        Some(Self(pane))
    }

    /// The pane at `rect` of the strip (its points, from its top left), just under it; hidden
    /// for none.
    pub(super) fn place(&self, strip: &WebView, rect: Option<(u32, u32, u32, u32)>) {
        let Some((x, y, width, height)) = rect else {
            // SAFETY: an NSView takes a BOOL.
            let _: () = unsafe { msg_send![&*self.0, setHidden: Bool::YES] };
            return;
        };
        let view = strip.webview();
        // SAFETY: an NSView answers its superview, or nil.
        let parent: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, superview] };
        let Some(parent) = parent else {
            return;
        };
        // SAFETY: an NSView answers its bounds, and whether its coordinates run downwards.
        let bounds: CGRect = unsafe { msg_send![&*parent, bounds] };
        // SAFETY: as above.
        let flipped: Bool = unsafe { msg_send![&*parent, isFlipped] };
        let (y, height) = (f64::from(y), f64::from(height));
        let frame = CGRect {
            origin: CGPoint {
                x: f64::from(x),
                y: if flipped.as_bool() {
                    y
                } else {
                    bounds.size.height - y - height
                },
            },
            size: CGSize {
                width: f64::from(width),
                height,
            },
        };
        // SAFETY: an NSView takes a frame.
        let _: () = unsafe { msg_send![&*self.0, setFrame: frame] };
        // Over a page built since: just under the strip again.
        let _ = below(&self.0, strip);
        // SAFETY: an NSView takes a BOOL.
        let _: () = unsafe { msg_send![&*self.0, setHidden: Bool::NO] };
    }
}
