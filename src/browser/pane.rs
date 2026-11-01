//! The glass under the strip's open list (macOS): the address field's suggestions and the
//! history menu are drawn by the strip's page, over the page shown, on the system's menu
//! material, rounded as a menu is. The page says where its list is (`glass x y w h`); the pane
//! follows it, hidden while none is open.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use wry::{WebView, WebViewExtMacOS};

use super::capture::{CGPoint, CGRect, CGSize};
use super::under::{below, material, shade};

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

    /// The pane in the appearance of the tab under it, as the strip's band.
    pub(super) fn shade(&self, dark: Option<bool>) {
        shade(&self.0, dark);
    }
}
