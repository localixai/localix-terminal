//! The terminal's canvas on Linux: a Cairo image surface of premultiplied colour, clear where
//! nothing is drawn (the terminal's ground is the view's to paint, never a row's), and the
//! strokes the shared drawing (`draw.rs`) makes in it: fills, outlines, a wave, dashes, and runs
//! of text (`text.rs`). Everything is in device pixels, the font taken at its size times the
//! screen's scale, so cells fall on whole pixels.

use gtk::cairo;

use super::super::draw::{Pen, Rect};
use super::super::layout::Run;
use super::super::painter::Backend;
use super::super::theme::Paint;
use super::fonts::Fonts;
use super::text;

/// A surface to draw a terminal's rows in, `width` by `height` pixels, and its context.
pub(crate) struct Canvas {
    surface: cairo::ImageSurface,
    context: cairo::Context,
    width: usize,
    height: usize,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let (w, h) = (i32::try_from(width).ok()?, i32::try_from(height).ok()?);
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
        let context = cairo::Context::new(&surface).ok()?;
        Some(Self {
            surface,
            context,
            width,
            height,
        })
    }

    fn source(&self, paint: Paint) {
        let part = |c: u8| f64::from(c) / 255.0;
        let Paint { rgb, alpha } = paint;
        (self.context).set_source_rgba(part(rgb.r), part(rgb.g), part(rgb.b), part(alpha));
    }

    fn stroke(&self, paint: Paint, width: f64) {
        self.source(paint);
        self.context.set_line_width(width);
        let _ = self.context.stroke();
    }
}

impl Pen for Canvas {
    type Fonts = Fonts;

    fn fill(&self, area: Rect, paint: Paint) {
        self.source(paint);
        (self.context).rectangle(area.x, area.y, area.width, area.height);
        let _ = self.context.fill();
    }

    fn outline(&self, area: Rect, paint: Paint, width: f64) {
        let inset = width / 2.0;
        let (x, y) = (area.x + inset, area.y + inset);
        (self.context).rectangle(x, y, area.width - width, area.height - width);
        self.stroke(paint, width);
    }

    fn wave(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint) {
        let step = (thick * 2.0).max(2.0);
        self.context.move_to(from, y);
        let mut x = from;
        let mut up = true;
        while x < from + span {
            let next = (x + step * 2.0).min(from + span);
            let peak = if up { y - thick } else { y + thick };
            (self.context).curve_to(x + step, peak, x + step, peak, next, y);
            x = next;
            up = !up;
        }
        self.stroke(paint, thick);
    }

    fn dashes(&self, from: f64, span: f64, y: f64, thick: f64, paint: Paint, dash: f64) {
        self.context.set_dash(&[dash, dash], 0.0);
        let middle = y + thick / 2.0;
        self.context.move_to(from, middle);
        self.context.line_to(from + span, middle);
        self.stroke(paint, thick);
        self.context.set_dash(&[], 0.0);
    }

    fn text(&self, fonts: &Fonts, run: &Run, at: (f64, f64), cell_width: f64) {
        text::draw(&self.context, fonts, run, at, cell_width);
    }

    fn width(&self) -> usize {
        self.width
    }
}

impl Backend for Canvas {
    type Image = cairo::ImageSurface;

    fn fonts(stack: &str, size: f64, ligatures: bool) -> Option<Fonts> {
        Fonts::new(stack, size, ligatures)
    }

    fn canvas(width: usize, height: usize) -> Option<Self> {
        Self::new(width, height)
    }

    fn image(&self) -> Option<cairo::ImageSurface> {
        self.surface.flush();
        Some(self.surface.clone())
    }

    fn height(&self) -> usize {
        self.height
    }

    fn rgba(&self) -> Vec<u8> {
        self.surface.flush();
        let stride = usize::try_from(self.surface.stride()).unwrap_or(0);
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        let width = self.width;
        let read = self.surface.with_data(|bytes| {
            for line in bytes.chunks_exact(stride.max(1)) {
                for pixel in line.as_chunks::<4>().0.iter().take(width) {
                    // A 32-bit word of alpha, red, green and blue, its low byte first.
                    out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
        });
        if read.is_err() {
            return vec![];
        }
        out
    }

    fn bytes(image: &cairo::ImageSurface) -> usize {
        let stride = usize::try_from(image.stride()).unwrap_or(0);
        stride * usize::try_from(image.height()).unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod tests;
