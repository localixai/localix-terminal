//! Pictures read and written as PNG by the system's image library (macOS): a terminal canvas's
//! pixels (red, green, blue, alpha, premultiplied) to a file and back, for the pixel tests'
//! pictures kept beside them.

use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;

use objc2_core_foundation::{CFData, CFMutableData, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow,
    CGBitmapContextGetData, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, kCGColorSpaceSRGB,
};

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageDestinationCreateWithData(
        data: *const CFMutableData,
        kind: *const CFString,
        count: usize,
        options: *const c_void,
    ) -> *mut CFType;
    fn CGImageDestinationAddImage(
        destination: *mut CFType,
        image: *const CGImage,
        props: *const c_void,
    );
    fn CGImageDestinationFinalize(destination: *mut CFType) -> bool;
    fn CGImageSourceCreateWithData(data: *const CFData, options: *const c_void) -> *mut CFType;
    fn CGImageSourceCreateImageAtIndex(
        source: *mut CFType,
        at: usize,
        options: *const c_void,
    ) -> *mut CGImage;
}

/// A picture: its size and its pixels, four bytes each.
pub(crate) struct Pixels {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgba: Vec<u8>,
}

/// A bitmap of `width` by `height` in sRGB, premultiplied, blue first.
fn bitmap(width: usize, height: usize) -> CFRetained<CGContext> {
    // SAFETY: the system's constant name of sRGB.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).unwrap();
    let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    // SAFETY: the context makes its own buffer.
    unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width,
            height,
            8,
            0,
            Some(&space),
            info,
        )
    }
    .unwrap()
}

/// The context's buffer, read as red, green, blue, alpha.
fn read(context: &CGContext, width: usize, height: usize) -> Vec<u8> {
    let stride = CGBitmapContextGetBytesPerRow(Some(context));
    let data = CGBitmapContextGetData(Some(context)).cast::<u8>();
    // SAFETY: the context's own buffer, `height` rows of `stride` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(data, stride * height) };
    let mut out = Vec::with_capacity(width * height * 4);
    for line in bytes.chunks_exact(stride) {
        for pixel in line.as_chunks::<4>().0.iter().take(width) {
            out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    out
}

/// Writes `pixels` to `path` as PNG.
pub(crate) fn write(path: &Path, pixels: &Pixels) {
    let context = bitmap(pixels.width, pixels.height);
    let stride = CGBitmapContextGetBytesPerRow(Some(&context));
    let data = CGBitmapContextGetData(Some(&context)).cast::<u8>();
    // SAFETY: the context's own buffer, written whole here.
    let bytes = unsafe { std::slice::from_raw_parts_mut(data, stride * pixels.height) };
    for (line, from) in bytes
        .chunks_exact_mut(stride)
        .zip(pixels.rgba.chunks_exact(pixels.width * 4))
    {
        for (to, from) in line
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(from.as_chunks::<4>().0.iter())
        {
            to.copy_from_slice(&[from[2], from[1], from[0], from[3]]);
        }
    }
    let image = CGBitmapContextCreateImage(Some(&context)).unwrap();
    let data = CFMutableData::new(None, 0).unwrap();
    let kind = CFString::from_str("public.png");
    // SAFETY: a destination over live data, of one image.
    let destination =
        unsafe { CGImageDestinationCreateWithData(&*data, &*kind, 1, std::ptr::null()) };
    let destination = NonNull::new(destination).unwrap();
    // SAFETY: a live destination and image.
    unsafe { CGImageDestinationAddImage(destination.as_ptr(), &*image, std::ptr::null()) };
    // SAFETY: as above.
    assert!(unsafe { CGImageDestinationFinalize(destination.as_ptr()) });
    // SAFETY: made above with one retain, handed over here.
    drop(unsafe { CFRetained::from_raw(destination) });
    std::fs::write(path, data.to_vec()).unwrap();
}

/// The PNG at `path`.
pub(crate) fn load(path: &Path) -> Pixels {
    let bytes = std::fs::read(path).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
    let data = CFData::from_bytes(&bytes);
    // SAFETY: a source over live data.
    let source =
        NonNull::new(unsafe { CGImageSourceCreateWithData(&*data, std::ptr::null()) }).unwrap();
    // SAFETY: the first image of a live source.
    let image = NonNull::new(unsafe {
        CGImageSourceCreateImageAtIndex(source.as_ptr(), 0, std::ptr::null())
    })
    .unwrap();
    // SAFETY: each made above with one retain, handed over here.
    let image = unsafe { CFRetained::from_raw(image) };
    // SAFETY: as above.
    drop(unsafe { CFRetained::from_raw(source) });
    let (width, height) = (CGImage::width(Some(&image)), CGImage::height(Some(&image)));
    let context = bitmap(width, height);
    let all = objc2_core_foundation::CGRect::new(
        objc2_core_foundation::CGPoint::new(0.0, 0.0),
        objc2_core_foundation::CGSize::new(width as f64, height as f64),
    );
    CGContext::draw_image(Some(&context), all, Some(&image));
    Pixels {
        width,
        height,
        rgba: read(&context, width, height),
    }
}
