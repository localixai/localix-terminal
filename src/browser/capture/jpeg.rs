//! The picture WebKit drew, as JPEG (macOS): what it takes from WebKit's image is taken on the
//! main thread, where the image is handed over, and the encoding, the slow part, on a thread of
//! its own, so the window's loop never waits for it.

use std::ffi::c_void;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{Encoding, RefEncode, class, msg_send};

use super::{CGPoint, CGRect, CGSize, QUALITY, Shot};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGImageRetain(image: *mut CGImage) -> *mut CGImage;
    fn CGImageRelease(image: *mut CGImage);
}

/// CoreGraphics' image, only ever behind a pointer.
#[repr(C)]
struct CGImage {
    _opaque: [u8; 0],
}

// SAFETY: a CGImageRef is a pointer to the opaque `struct CGImage`, as the encoding says.
unsafe impl RefEncode for CGImage {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGImage", &[]));
}

/// What the encoding needs, apart from WebKit's image: its pixels, and the ground a clear pixel
/// is laid on.
pub(super) struct Picture {
    /// A CGImage, retained (released when this goes).
    image: *mut CGImage,
    /// The window's background, in the appearance in force when the picture came, as fixed
    /// components: a system colour read on another thread would take that thread's appearance.
    ground: Option<Retained<AnyObject>>,
}

// SAFETY: a CGImage is immutable and may be used from any thread; the colour is one of fixed
// components (`take`), immutable too. Neither is used by two threads at once.
unsafe impl Send for Picture {}

impl Drop for Picture {
    fn drop(&mut self) {
        // SAFETY: the image `take` retained, released once, here.
        unsafe { CGImageRelease(self.image) };
    }
}

/// What `image` holds, taken at its full size in pixels (its largest representation's, as its
/// TIFF holds it).
///
/// # Safety
/// `image` is a live NSImage, on the main thread, and an autorelease pool is in place.
pub(super) unsafe fn take(image: *mut AnyObject) -> Result<Picture, String> {
    // SAFETY: `image` is a live NSImage (the caller's contract); it answers an NSArray.
    let reps: *mut AnyObject = unsafe { msg_send![image, representations] };
    if reps.is_null() {
        return Err("the picture has no bitmap".into());
    }
    // SAFETY: `reps` is a live NSArray; it answers its count.
    let count: usize = unsafe { msg_send![reps, count] };
    let (mut wide, mut high) = (0_isize, 0_isize);
    for at in 0..count {
        // SAFETY: `at` is below the count; the array answers a live NSImageRep.
        let rep: *mut AnyObject = unsafe { msg_send![reps, objectAtIndex: at] };
        // SAFETY: an NSImageRep answers its width in pixels, an NSInteger.
        let w: isize = unsafe { msg_send![rep, pixelsWide] };
        // SAFETY: as above, its height.
        let h: isize = unsafe { msg_send![rep, pixelsHigh] };
        if w > wide {
            (wide, high) = (w, h);
        }
    }
    if wide <= 0 || high <= 0 {
        return Err("the picture has no bitmap".into());
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a picture's pixels, far below 2^52"
    )]
    let mut rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: wide as f64,
            height: high as f64,
        },
    };
    let none = std::ptr::null_mut::<AnyObject>();
    // SAFETY: an NSImage takes a rect (in place, here one of its pixels, so none is lost to
    // scaling), a nil context and nil hints, and answers a CGImage it owns, or null.
    let image: *mut CGImage = unsafe {
        msg_send![image, CGImageForProposedRect: &raw mut rect, context: none, hints: none]
    };
    if image.is_null() {
        return Err("the picture could not be decoded".into());
    }
    // SAFETY: a live CGImage the NSImage owns; retained, it outlives the image.
    let image = unsafe { CGImageRetain(image) };
    Ok(Picture {
        image,
        ground: ground(),
    })
}

/// The window's background colour, in the appearance in force now, as fixed sRGB components.
fn ground() -> Option<Retained<AnyObject>> {
    // SAFETY: NSColor's class method answers the shared, live system colour.
    let system: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(NSColor), windowBackgroundColor] };
    // SAFETY: NSColorSpace's class method answers the shared sRGB space.
    let srgb: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(NSColorSpace), sRGBColorSpace] };
    // SAFETY: an NSColor takes a colour space and answers itself in it, resolved, or nil.
    unsafe { msg_send![&*system?, colorUsingColorSpace: &*srgb?] }
}

impl Picture {
    /// The picture as JPEG, at `QUALITY`, clear pixels on the window's ground. Any thread.
    pub(super) fn encode(self) -> Result<Shot, String> {
        objc2::rc::autoreleasepool(|_| self.encoded())
    }

    fn encoded(&self) -> Result<Shot, String> {
        /// NSBitmapImageFileTypeJPEG.
        const JPEG: usize = 3;
        // SAFETY: `alloc` on NSBitmapImageRep answers an uninitialised instance.
        let alloc: Allocated<AnyObject> = unsafe { msg_send![class!(NSBitmapImageRep), alloc] };
        // SAFETY: a bitmap takes a live CGImage (retained by `self`) and answers itself, or nil.
        let bitmap: Option<Retained<AnyObject>> =
            unsafe { msg_send![alloc, initWithCGImage: self.image] };
        let bitmap = bitmap.ok_or("the picture could not be decoded")?;
        let key = super::super::appkit::text("NSImageCompressionFactor");
        // SAFETY: NSNumber's class method takes a double and returns a number.
        let quality: *mut AnyObject =
            unsafe { msg_send![class!(NSNumber), numberWithDouble: QUALITY] };
        // SAFETY: `quality` and `key` are live objects, made just above in this pool.
        let properties: *mut AnyObject = unsafe {
            msg_send![class!(NSMutableDictionary), dictionaryWithObject: quality, forKey: &*key]
        };
        // A page on the window's glass is clear where the glass shows, and a JPEG has no clear: it
        // is laid on the window's own background, in the appearance in force, not on black.
        let fallback = super::super::appkit::text("NSImageFallbackBackgroundColor");
        if let (false, Some(ground)) = (properties.is_null(), self.ground.as_ref()) {
            // SAFETY: `properties` is a live NSMutableDictionary, `ground` and `fallback` live.
            let _: () = unsafe { msg_send![properties, setObject: &**ground, forKey: &*fallback] };
        }
        // SAFETY: `bitmap` is a live NSBitmapImageRep, `properties` a dictionary (or nil).
        let data: *mut AnyObject =
            unsafe { msg_send![&*bitmap, representationUsingType: JPEG, properties: properties] };
        if data.is_null() {
            return Err("the picture could not be encoded as JPEG".into());
        }
        // SAFETY: `data` is a live NSData, checked non-null above.
        let bytes: *const c_void = unsafe { msg_send![data, bytes] };
        // SAFETY: as above; an NSData answers its length as an NSUInteger.
        let length: usize = unsafe { msg_send![data, length] };
        // SAFETY: `bitmap` is a live NSBitmapImageRep; it answers an NSInteger.
        let width: isize = unsafe { msg_send![&*bitmap, pixelsWide] };
        // SAFETY: as above.
        let height: isize = unsafe { msg_send![&*bitmap, pixelsHigh] };
        // SAFETY: `bytes` points at `length` bytes the NSData owns, alive while the pool is; they
        // are copied out before it drains.
        let jpeg = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length) }.to_vec();
        Ok(Shot {
            jpeg,
            width: u32::try_from(width.max(0)).unwrap_or(u32::MAX),
            height: u32::try_from(height.max(0)).unwrap_or(u32::MAX),
        })
    }
}
