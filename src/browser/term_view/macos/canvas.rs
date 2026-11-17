//! The terminal's canvas on macOS: a Core Graphics bitmap of premultiplied colour, clear where
//! nothing is drawn (the terminal's ground is never painted, so the window's glass shows
//! through), and the strokes the shared drawing (`draw.rs`) makes in it: fills, outlines, a
//! wave, dashes, and runs of text (`text.rs`). Everything is in device pixels, the font taken at
//! its size times the screen's scale, so cells fall on whole pixels.

use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow,
    CGBitmapContextGetData, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, kCGColorSpaceSRGB,
};

use super::super::draw::{Pen, Rect};
use super::super::layout::Run;
use super::super::painter::Backend;
use super::super::theme::Paint;
use super::fonts::Fonts;
use super::text;

/// A bitmap to draw a terminal's rows in, `width` by `height` pixels.
pub(crate) struct Canvas {
    context: CFRetained<CGContext>,
    width: usize,
    height: usize,
}

// SAFETY: the context is this canvas's alone and is drawn in by one thread at a time.
unsafe impl Send for Canvas {}

impl Canvas {
    /// A clear bitmap of `width` by `height` pixels; none for an empty one.
    fn new(width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        // SAFETY: the name is Core Graphics' constant, alive for the process.
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
        let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
        // SAFETY: no buffer: the context makes and owns its own, of the rows it needs.
        let context = unsafe {
            CGBitmapContextCreate(
                std::ptr::null_mut(),
                width,
                height,
                8,
                0,
                Some(&space),
                info,
            )
        }?;
        // A context's y grows up; a terminal's grows down.
        CGContext::translate_ctm(Some(&context), 0.0, height as f64);
        CGContext::scale_ctm(Some(&context), 1.0, -1.0);
        CGContext::set_should_smooth_fonts(Some(&context), false);
        CGContext::set_allows_antialiasing(Some(&context), true);
        Some(Self {
            context,
            width,
            height,
        })
    }

    pub(super) fn context(&self) -> &CGContext {
        &self.context
    }

    /// The pixels, a row after another, each red, green, blue and alpha, premultiplied.
    fn pixels(&self) -> Vec<u8> {
        let stride = CGBitmapContextGetBytesPerRow(Some(&self.context));
        let data = CGBitmapContextGetData(Some(&self.context)).cast::<u8>();
        if data.is_null() {
            return vec![];
        }
        // SAFETY: the context's own buffer, `height` rows of `stride` bytes, not drawn in
        // meanwhile (this canvas is borrowed).
        let bytes = unsafe { std::slice::from_raw_parts(data, stride * self.height) };
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for line in bytes.chunks_exact(stride) {
            for pixel in line.as_chunks::<4>().0.iter().take(self.width) {
                // Stored as blue, green, red, alpha (32 bits, little end first).
                out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        out
    }
}

impl Pen for Canvas {
    type Fonts = Fonts;

    fn fill(&self, area: Rect, paint: Paint) {
        set_fill(&self.context, paint);
        CGContext::fill_rect(Some(&self.context), cg(area));
    }

    fn outline(&self, area: Rect, paint: Paint, width: f64) {
        set_stroke(&self.context, paint, width);
        let inset = width / 2.0;
        let inner = rect(
            area.x + inset,
            area.y + inset,
            area.width - width,
            area.height - width,
        );
        CGContext::stroke_rect(Some(&self.context), inner);
    }

    fn wave(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint) {
        let context = &self.context;
        set_stroke(context, paint, thick);
        let step = (thick * 2.0).max(2.0);
        CGContext::begin_path(Some(context));
        CGContext::move_to_point(Some(context), from, y);
        let mut x = from;
        let mut up = true;
        while x < from + span {
            let next = (x + step * 2.0).min(from + span);
            let peak = if up { y - thick } else { y + thick };
            CGContext::add_curve_to_point(Some(context), x + step, peak, x + step, peak, next, y);
            x = next;
            up = !up;
        }
        CGContext::stroke_path(Some(context));
    }

    fn dashes(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint, dash: f64) {
        let context = &self.context;
        set_stroke(context, paint, thick);
        let lengths = [dash, dash];
        // SAFETY: two lengths, alive through the call.
        unsafe { CGContext::set_line_dash(Some(context), 0.0, lengths.as_ptr(), lengths.len()) };
        let middle = y + thick / 2.0;
        let ends = [
            CGPoint { x: from, y: middle },
            CGPoint {
                x: from + span,
                y: middle,
            },
        ];
        // SAFETY: two points, one segment, alive through the call.
        unsafe { CGContext::stroke_line_segments(Some(context), ends.as_ptr(), ends.len()) };
        // SAFETY: no dashes again for what is stroked next.
        unsafe { CGContext::set_line_dash(Some(context), 0.0, std::ptr::null(), 0) };
    }

    fn text(&self, fonts: &Fonts, run: &Run, (left, baseline): (f64, f64), cell_width: f64) {
        CGContext::save_g_state(Some(&self.context));
        CGContext::translate_ctm(Some(&self.context), left, 0.0);
        text::draw(&self.context, fonts, run, (cell_width, baseline));
        CGContext::restore_g_state(Some(&self.context));
    }

    fn width(&self) -> usize {
        self.width
    }
}

impl Backend for Canvas {
    type Image = CFRetained<CGImage>;

    fn fonts(stack: &str, size: f64, ligatures: bool) -> Option<Fonts> {
        Fonts::new(stack, size, ligatures)
    }

    fn canvas(width: usize, height: usize) -> Option<Self> {
        Self::new(width, height)
    }

    fn image(&self) -> Option<Self::Image> {
        CGBitmapContextCreateImage(Some(&self.context))
    }

    fn height(&self) -> usize {
        self.height
    }

    fn rgba(&self) -> Vec<u8> {
        self.pixels()
    }

    fn bytes(image: &Self::Image) -> usize {
        CGImage::bytes_per_row(Some(image)) * CGImage::height(Some(image))
    }
}

fn cg(area: Rect) -> CGRect {
    rect(area.x, area.y, area.width, area.height)
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> CGRect {
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

fn channel(c: u8) -> f64 {
    f64::from(c) / 255.0
}

fn set_fill(context: &CGContext, paint: Paint) {
    let Paint { rgb, alpha } = paint;
    let (r, g, b, a) = (
        channel(rgb.r),
        channel(rgb.g),
        channel(rgb.b),
        channel(alpha),
    );
    CGContext::set_rgb_fill_color(Some(context), r, g, b, a);
}

fn set_stroke(context: &CGContext, paint: Paint, width: f64) {
    let Paint { rgb, alpha } = paint;
    let (r, g, b, a) = (
        channel(rgb.r),
        channel(rgb.g),
        channel(rgb.b),
        channel(alpha),
    );
    CGContext::set_rgb_stroke_color(Some(context), r, g, b, a);
    CGContext::set_line_width(Some(context), width);
}
