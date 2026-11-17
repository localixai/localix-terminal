//! The AppKit messages the terminal's view sends again and again, each written once: a view's
//! window, bounds and scale, whether its window is the key one, and what a key or pointer event
//! says of itself.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSEvent;
use objc2_core_foundation::{CGPoint, CGRect};
use objc2_foundation::NSString;

/// `view`'s window, if it is in one.
pub(super) fn window(view: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: an NSView answers its window, or nil.
    unsafe { msg_send![view, window] }
}

/// The screen's pixels to a point where `view` is: its window's, else a Retina screen's.
pub(super) fn scale(view: &AnyObject) -> f64 {
    window(view).map_or(2.0, |window| {
        // SAFETY: a window answers its backing scale, a CGFloat.
        unsafe { msg_send![&*window, backingScaleFactor] }
    })
}

/// Whether `view`'s window is the key one.
pub(super) fn key_window(view: &AnyObject) -> bool {
    window(view).is_some_and(|window| {
        // SAFETY: a window answers whether it is the key one, a BOOL.
        unsafe { msg_send![&*window, isKeyWindow] }
    })
}

pub(super) fn bounds(view: &AnyObject) -> CGRect {
    // SAFETY: an NSView answers its bounds, a CGRect.
    unsafe { msg_send![view, bounds] }
}

/// `view`'s superview, if it has one.
pub(super) fn superview(view: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: an NSView answers its superview, or nil.
    unsafe { msg_send![view, superview] }
}

/// The text an NSString holds; nothing for none.
pub(super) fn text(string: Option<Retained<NSString>>) -> String {
    string.map(|string| string.to_string()).unwrap_or_default()
}

pub(super) fn flags(event: &NSEvent) -> usize {
    // SAFETY: an event answers its modifier flags, an NSUInteger.
    unsafe { msg_send![event, modifierFlags] }
}

pub(super) fn key_code(event: &NSEvent) -> u16 {
    // SAFETY: a key event answers its virtual key code, an unsigned short.
    unsafe { msg_send![event, keyCode] }
}

pub(super) fn repeats(event: &NSEvent) -> bool {
    // SAFETY: a key event answers whether it repeats, a BOOL.
    unsafe { msg_send![event, isARepeat] }
}

/// The characters a key event makes with `flags` held instead of its own.
pub(super) fn chars_with(event: &NSEvent, flags: usize) -> String {
    // SAFETY: a key event answers its characters with other modifiers, an NSString.
    text(unsafe { msg_send![event, charactersByApplyingModifiers: flags] })
}

pub(super) fn button(event: &NSEvent) -> isize {
    // SAFETY: a mouse event answers its button's number, an NSInteger.
    unsafe { msg_send![event, buttonNumber] }
}

/// When the event came, in seconds since the system started.
pub(super) fn time(event: &NSEvent) -> f64 {
    // SAFETY: an event answers its timestamp, an NSTimeInterval.
    unsafe { msg_send![event, timestamp] }
}

/// The wheel's or the trackpad's turn down the view, and whether it is in points.
pub(super) fn turn(event: &NSEvent) -> (f64, bool) {
    // SAFETY: a scroll event answers its turn, a CGFloat.
    let dy: f64 = unsafe { msg_send![event, scrollingDeltaY] };
    // SAFETY: and whether that turn is in points, a BOOL.
    let precise: bool = unsafe { msg_send![event, hasPreciseScrollingDeltas] };
    (dy, precise)
}

/// Where the event is in `view`, in its points from its top left (the view counts from the top).
pub(super) fn at(view: &AnyObject, event: &NSEvent) -> CGPoint {
    // SAFETY: an event answers its place in its window, a CGPoint.
    let point: CGPoint = unsafe { msg_send![event, locationInWindow] };
    // SAFETY: a view converts a point from its window's (nil) into its own.
    unsafe { msg_send![view, convertPoint: point, fromView: std::ptr::null::<AnyObject>()] }
}

/// The system's accent colour as `view` draws it (its appearance's), in sRGB; the token sheet's
/// where the system gives none.
pub(super) fn accent(view: &AnyObject) -> crate::surface::Rgb {
    let read = std::sync::Arc::new(std::sync::Mutex::new(None));
    let into = std::sync::Arc::clone(&read);
    let block = block2::RcBlock::new(move || *crate::lock(&into) = accent_now());
    // SAFETY: an NSView answers its effective appearance.
    let appearance: Retained<AnyObject> = unsafe { msg_send![view, effectiveAppearance] };
    // SAFETY: an appearance runs a block with itself as the one colours are read in.
    let _: () = unsafe { msg_send![&*appearance, performAsCurrentDrawingAppearance: &*block] };
    let read = *crate::lock(&read);
    read.unwrap_or(super::super::theme::ACCENT)
}

/// The system's accent colour in the appearance colours are read in now, in sRGB.
fn accent_now() -> Option<crate::surface::Rgb> {
    use objc2::class;
    // SAFETY: NSColor answers the accent the person chose.
    let accent: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(NSColor), controlAccentColor] };
    // SAFETY: NSColorSpace answers sRGB's.
    let space: Retained<AnyObject> = unsafe { msg_send![class!(NSColorSpace), sRGBColorSpace] };
    // SAFETY: a colour answers itself in another space, or nil where it has none there.
    let srgb: Option<Retained<AnyObject>> =
        unsafe { msg_send![&*accent?, colorUsingColorSpace: &*space] };
    let srgb = srgb?;
    let channel = |value: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a colour's channel, held to a byte's range just before"
        )]
        let byte = (value * 255.0).round().clamp(0.0, 255.0) as u8;
        byte
    };
    // SAFETY: an sRGB colour answers its components, CGFloats (this and the next two).
    let r: f64 = unsafe { msg_send![&*srgb, redComponent] };
    // SAFETY: as above.
    let g: f64 = unsafe { msg_send![&*srgb, greenComponent] };
    // SAFETY: as above.
    let b: f64 = unsafe { msg_send![&*srgb, blueComponent] };
    Some(crate::surface::Rgb {
        r: channel(r),
        g: channel(g),
        b: channel(b),
    })
}
