//! A picture of the page as the WebView drew it: the compositor's own pixels (a canvas, WebGL and
//! video included), not the DOM drawn again. JPEG, at most `MAX_EDGE` pixels on its long side, so
//! a screenshot costs the model about as much as a long page outline.

#[cfg(not(target_os = "linux"))]
use std::sync::mpsc::SyncSender;

#[cfg(target_os = "linux")]
mod gtk;
#[cfg(target_os = "macos")]
mod jpeg;
#[cfg(target_os = "linux")]
pub(super) use gtk::capture;

/// The long side of a picture, in pixels.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "linux", test)),
    expect(
        dead_code,
        reason = "the compositor's picture is taken on macOS and Linux; its tests use it everywhere"
    )
)]
pub(super) const MAX_EDGE: f64 = 1280.0;
/// JPEG quality: text stays legible, a full page stays near 150 KB.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "linux")),
    expect(
        dead_code,
        reason = "the compositor's picture is taken on macOS and Linux"
    )
)]
const QUALITY: f64 = 0.7;

/// A part of the page, in CSS pixels from the viewport's top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// An encoded picture.
#[derive(Debug, Clone)]
pub struct Shot {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The size to draw `width`×`height` at so its long side is at most `MAX_EDGE`.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "linux", test)),
    expect(
        dead_code,
        reason = "the compositor's picture is taken on macOS and Linux; its tests use it everywhere"
    )
)]
pub(super) fn fitted(width: f64, height: f64) -> (f64, f64) {
    let scale = (MAX_EDGE / width.max(height)).min(1.0);
    ((width * scale).round(), (height * scale).round())
}

/// Take the picture of `clip` (or of the whole view) and send it to `reply` once WebKit has drawn
/// it. Runs on the main thread, where the WebView lives; the answer comes later, from there too.
#[cfg(target_os = "macos")]
pub(super) fn capture(
    webview: &wry::WebView,
    clip: Option<Clip>,
    reply: SyncSender<Result<Shot, String>>,
) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use wry::WebViewExtMacOS;

    let view = webview.webview();
    let window = webview.ns_window();
    // SAFETY: the WKWebView is an NSView; `bounds` answers an NSRect, which `CGRect` lays out.
    let bounds: CGRect = unsafe { msg_send![&*view, bounds] };
    // SAFETY: wry's NSWindow is alive while its WebView is; it answers a CGFloat.
    let scale: f64 = unsafe { msg_send![&*window, backingScaleFactor] };
    let rect = match clip {
        Some(clip) => CGRect {
            origin: CGPoint {
                x: clip.x,
                y: clip.y,
            },
            size: CGSize {
                width: clip.width,
                height: clip.height,
            },
        },
        None => CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: bounds.size.width,
                height: bounds.size.height,
            },
        },
    };
    // The snapshot's width is in points; the bitmap has the screen's scale on top of it.
    let (width, _) = fitted(rect.size.width * scale, rect.size.height * scale);
    // SAFETY: `new` on WKSnapshotConfiguration returns a retained instance.
    let configuration: Retained<AnyObject> =
        unsafe { msg_send![class!(WKSnapshotConfiguration), new] };
    // SAFETY: a snapshot configuration takes an NSRect, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*configuration, setRect: rect] };
    // SAFETY: NSNumber's class method takes a double and returns a number.
    let points: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNumber), numberWithDouble: width / scale.max(1.0)] };
    // SAFETY: a snapshot configuration takes an NSNumber width, which it retains.
    let _: () = unsafe { msg_send![&*configuration, setSnapshotWidth: &*points] };
    // SAFETY: a snapshot configuration takes a BOOL.
    let _: () = unsafe { msg_send![&*configuration, setAfterScreenUpdates: Bool::YES] };
    let done = RcBlock::new(move |image: *mut AnyObject, error: *mut AnyObject| {
        let picture = if image.is_null() {
            Err(match error.is_null() {
                true => "WebKit returned no picture".to_string(),
                // SAFETY: WebKit hands a live NSError for the length of the callback.
                false => unsafe { describe(error) },
            })
        } else {
            // SAFETY: WebKit hands a live NSImage for the length of the callback, on the main
            // thread, and what `take` reads of it is autoreleased into this pool.
            objc2::rc::autoreleasepool(|_| unsafe { jpeg::take(image) })
        };
        // Encoded apart: a picture of a long page takes the main thread a while to encode.
        let encoded = picture.map(|picture| {
            let reply = reply.clone();
            std::thread::Builder::new()
                .name("desk-picture".into())
                .spawn(move || drop(reply.send(picture.encode())))
        });
        match encoded {
            Ok(Ok(_)) => {}
            Ok(Err(why)) => drop(reply.send(Err(format!("the picture was not encoded: {why}")))),
            Err(why) => drop(reply.send(Err(why))),
        }
    });
    // SAFETY: a WKWebView takes a configuration and a block of (NSImage, NSError), which it
    // copies and calls once on the main thread.
    let _: () = unsafe {
        msg_send![
            &*view,
            takeSnapshotWithConfiguration: &*configuration,
            completionHandler: &*done
        ]
    };
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[expect(
    clippy::needless_pass_by_value,
    reason = "one signature on every system: macOS hands the reply to WebKit's completion handler"
)]
pub(super) fn capture(
    _webview: &wry::WebView,
    _clip: Option<Clip>,
    reply: SyncSender<Result<Shot, String>>,
) {
    let _ = reply.send(Err(
        "screenshots are not supported on this system yet (macOS and Linux only); use `read` or `script`"
            .into(),
    ));
}

/// An NSError's description.
///
/// # Safety
/// `error` is a live NSError.
#[cfg(target_os = "macos")]
unsafe fn describe(error: *mut objc2::runtime::AnyObject) -> String {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    // SAFETY: `error` is a live NSError (the caller's contract); it answers an NSString.
    let text: *mut AnyObject = unsafe { msg_send![error, localizedDescription] };
    // SAFETY: `text` is a live NSString; its UTF-8 lives as long as it does, or is null.
    let utf8: *const std::ffi::c_char = unsafe { msg_send![text, UTF8String] };
    if utf8.is_null() {
        return "WebKit could not take the picture".into();
    }
    // SAFETY: `utf8` is non-null and NUL-terminated, alive while `text` is, and copied out here.
    unsafe { std::ffi::CStr::from_ptr(utf8) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[cfg(target_os = "macos")]
mod encoding {
    use objc2::{Encode, Encoding, RefEncode};

    use super::{CGPoint, CGRect, CGSize};

    // SAFETY: `CGPoint` is `repr(C)`, two f64s, as the encoding says.
    unsafe impl Encode for CGPoint {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    // SAFETY: `CGSize` is `repr(C)`, two f64s, as the encoding says.
    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    // SAFETY: `CGRect` is `repr(C)`, a `CGPoint` then a `CGSize`, as the encoding says.
    unsafe impl Encode for CGRect {
        const ENCODING: Encoding =
            Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
    }
    // SAFETY: a pointer to a `CGRect`, as the encoding says (an image's proposed rect).
    unsafe impl RefEncode for CGRect {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
