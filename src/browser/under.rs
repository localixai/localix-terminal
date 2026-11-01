//! Pages that run under the strip, as Safari's do (macOS 26): a page fills the window from its
//! top, WebKit told the strip covers its first `top` points (`obscuredContentInsets`), so its
//! layout viewport, its fixed headers and what the agent reads start below the strip as before,
//! while what scrolls up shows through the strip, blurred by a band of AppKit's material. Where
//! WebKit has no such inset, a page stays below the strip.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{Encode, Encoding, class, msg_send, sel};
use wry::{WebView, WebViewExtMacOS};

use super::capture::{CGPoint, CGRect, CGSize};

/// NSEdgeInsets: top, left, bottom, right.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Insets {
    top: f64,
    left: f64,
    bottom: f64,
    right: f64,
}

// SAFETY: `Insets` is `repr(C)`, four f64s, as NSEdgeInsets is and the encoding says.
unsafe impl Encode for Insets {
    const ENCODING: Encoding = Encoding::Struct(
        "NSEdgeInsets",
        &[f64::ENCODING, f64::ENCODING, f64::ENCODING, f64::ENCODING],
    );
}

/// Tell `webview` the strip covers its first `top` points; false where WebKit cannot be told,
/// and the page must stay below the strip.
pub(super) fn inset(webview: &WebView, top: f64) -> bool {
    let view = webview.webview();
    // SAFETY: the WKWebView is a live NSObject; `respondsToSelector:` answers a BOOL.
    let able: bool =
        unsafe { msg_send![&*view, respondsToSelector: sel!(setObscuredContentInsets:)] };
    if able {
        let insets = Insets {
            top,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        };
        // SAFETY: the view answers this selector (checked above); it takes an NSEdgeInsets.
        let _: () = unsafe { msg_send![&*view, setObscuredContentInsets: insets] };
    }
    able
}

/// What of `view` the strip covers: the points at its top (0 where none does).
pub(super) fn covered(view: &AnyObject) -> f64 {
    // SAFETY: as above.
    let able: bool = unsafe { msg_send![view, respondsToSelector: sel!(obscuredContentInsets)] };
    if !able {
        return 0.0;
    }
    // SAFETY: the view answers this selector (checked above), with an NSEdgeInsets.
    let insets: Insets = unsafe { msg_send![view, obscuredContentInsets] };
    insets.top
}

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

/// `view` in the light or dark appearance (`dark`), or its window's (none).
pub(super) fn shade(view: &AnyObject, dark: Option<bool>) {
    let name = match dark {
        Some(true) => Some(c"NSAppearanceNameDarkAqua"),
        Some(false) => Some(c"NSAppearanceNameAqua"),
        None => None,
    };
    // SAFETY: a NUL-terminated name, alive through the call; it answers a new string.
    let name: Option<Retained<AnyObject>> = name.and_then(|name| unsafe {
        msg_send![class!(NSString), stringWithUTF8String: name.as_ptr()]
    });
    // SAFETY: NSAppearance answers the system's appearance of a name, or nil.
    let appearance: Option<Retained<AnyObject>> =
        name.and_then(|name| unsafe { msg_send![class!(NSAppearance), appearanceNamed: &*name] });
    // SAFETY: an NSView takes an appearance, or nil for its window's.
    let _: () = unsafe { msg_send![view, setAppearance: appearance.as_deref()] };
}

/// The blur under the strip: AppKit's material over what of the window is beneath it (a page
/// scrolled under the strip), held to the window's top.
pub(super) struct Band(Retained<AnyObject>);

impl Band {
    /// The band behind `strip`, in the strip's own view.
    pub(super) fn behind(strip: &WebView) -> Option<Self> {
        /// NSVisualEffectMaterialHeaderView.
        const HEADER: isize = 10;
        /// NSAutoresizingMaskOptions: as wide as the window, held to its top.
        const FOLLOW: usize = 2 | 8;
        let band = material(HEADER);
        // SAFETY: an NSView takes an NSAutoresizingMaskOptions.
        let _: () = unsafe { msg_send![&*band, setAutoresizingMask: FOLLOW] };
        below(&band, strip)?;
        Some(Self(band))
    }

    /// The band in the appearance of the tab under it (`dark`; none known: the window's): its
    /// material is light over a light page, so the strip's words, inked for the page, read.
    pub(super) fn shade(&self, dark: Option<bool>) {
        shade(&self.0, dark);
    }

    /// The band as tall as the strip's `top` points, and the strip, then the band, over every
    /// page: a page built later is added above them.
    pub(super) fn lay(&self, strip: &WebView, top: f64) {
        let view = strip.webview();
        // SAFETY: an NSView answers its superview, or nil.
        let parent: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, superview] };
        let Some(parent) = parent else {
            return;
        };
        // SAFETY: an NSView answers its bounds.
        let bounds: CGRect = unsafe { msg_send![&*parent, bounds] };
        // SAFETY: an NSView answers whether its coordinates run downwards.
        let flipped: Bool = unsafe { msg_send![&*parent, isFlipped] };
        let y = if flipped.as_bool() {
            0.0
        } else {
            bounds.size.height - top
        };
        let frame = CGRect {
            origin: CGPoint { x: 0.0, y },
            size: CGSize {
                width: bounds.size.width,
                height: top,
            },
        };
        // SAFETY: an NSView takes a frame.
        let _: () = unsafe { msg_send![&*self.0, setFrame: frame] };
        // SAFETY: an NSView answers its subviews, an NSArray.
        let subviews: Retained<AnyObject> = unsafe { msg_send![&*parent, subviews] };
        // SAFETY: an NSArray answers its last object, or nil.
        let last: Option<Retained<AnyObject>> = unsafe { msg_send![&*subviews, lastObject] };
        let strip_view: &AnyObject = &view;
        if last.is_some_and(|last| std::ptr::eq(&*last, strip_view)) {
            return;
        }
        // A sibling added again goes on top (NSWindowAbove, 1, relative to none): the band,
        // then the strip over it.
        // SAFETY: an NSView adds (here: moves) a subview to the top of its siblings.
        let _: () = unsafe {
            msg_send![&*parent, addSubview: &*self.0, positioned: 1_isize, relativeTo: std::ptr::null::<AnyObject>()]
        };
        // SAFETY: as above.
        let _: () = unsafe {
            msg_send![&*parent, addSubview: &*view, positioned: 1_isize, relativeTo: std::ptr::null::<AnyObject>()]
        };
    }
}
