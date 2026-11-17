//! The terminal's fonts on Linux: the person's code font, read from its stack of names (the
//! Settings' CSS stack: the first family fontconfig has, else the system's `monospace`, which
//! fontconfig always names one for), at its size in pixels, in its four faces, through Pango,
//! which falls back on fontconfig's fonts for what a face lacks (emoji, other scripts); and the
//! cell its characters fill: the advance of a `W` wide and the font's line tall, as a terminal
//! page measured it. With ligatures off, every run is laid out with its joining features off
//! (`liga`, and `calt`, where code fonts keep their arrows).

use gtk::cairo;
use gtk::pango::{
    self,
    prelude::{FontFamilyExt, FontMapExt},
};

use super::super::draw::{Faces, Metrics};

/// One font in its four faces, the Pango context runs are laid out in, and its measures in
/// pixels.
pub(crate) struct Fonts {
    pub(super) context: pango::Context,
    faces: [pango::FontDescription; 4],
    /// The features each run is laid out with: none, or the joining ones off.
    pub(super) features: pango::AttrList,
    metrics: Metrics,
}

/// Pango's units in a pixel.
pub(super) const UNITS: f64 = pango::SCALE as f64;

impl Fonts {
    /// The first family of `stack` that is installed, at `size` pixels, or the system's
    /// monospace; its ligatures joining or not. None where Pango finds no font at all.
    pub(super) fn new(stack: &str, size: f64, ligatures: bool) -> Option<Self> {
        let map = pangocairo::FontMap::default();
        let installed: Vec<String> = (map.list_families().iter())
            .map(|family| family.name().to_lowercase())
            .collect();
        let family = (stack.split(','))
            .map(|name| name.trim().trim_matches(['"', '\'']))
            .find(|name| !name.is_empty() && installed.contains(&name.to_lowercase()))
            .unwrap_or("monospace");
        let context = map.create_context();
        let mut options = cairo::FontOptions::new().ok()?;
        // Grey edges: the rows are drawn on a clear ground, where colour fringes would show.
        options.set_antialias(cairo::Antialias::Gray);
        pangocairo::functions::context_set_font_options(&context, Some(&options));
        let mut regular = pango::FontDescription::new();
        regular.set_family(family);
        regular.set_absolute_size((size * UNITS).round());
        let face = |weight: pango::Weight, style: pango::Style| {
            let mut face = regular.clone();
            face.set_weight(weight);
            face.set_style(style);
            face
        };
        let (bold, italic) = (pango::Weight::Bold, pango::Style::Italic);
        let faces = [
            regular.clone(),
            face(bold, pango::Style::Normal),
            face(pango::Weight::Normal, italic),
            face(bold, italic),
        ];
        let features = pango::AttrList::new();
        if !ligatures {
            features.insert(pango::AttrFontFeatures::new("liga 0, calt 0"));
        }
        let metrics = measured(&context, &regular, size)?;
        Some(Self {
            context,
            faces,
            features,
            metrics,
        })
    }

    /// The face for bold and italic text.
    pub(super) fn face(&self, bold: bool, italic: bool) -> &pango::FontDescription {
        let [regular, bold_face, italic_face, both] = &self.faces;
        match (bold, italic) {
            (false, false) => regular,
            (true, false) => bold_face,
            (false, true) => italic_face,
            (true, true) => both,
        }
    }
}

impl Faces for Fonts {
    fn metrics(&self) -> Metrics {
        self.metrics
    }
}

/// The measures of `face`: a cell a `W` wide and the font's ascent and descent tall, the
/// baseline in the middle of the room left, the underline and the strike where the font has them.
fn measured(context: &pango::Context, face: &pango::FontDescription, size: f64) -> Option<Metrics> {
    use gtk::pango::prelude::FontExt;
    let font = context.load_font(face)?;
    let metrics = font.metrics(None);
    let px = |units: i32| f64::from(units) / UNITS;
    let (ascent, descent) = (px(metrics.ascent()), px(metrics.descent()));
    let height = (ascent + descent).ceil().max(1.0);
    let layout = pango::Layout::new(context);
    layout.set_font_description(Some(face));
    layout.set_text("W");
    let width = px(layout.extents().1.width());
    let width = if width > 0.0 { width } else { size * 0.6 };
    Some(Metrics {
        cell: (width, height),
        baseline: ((height - ascent - descent) / 2.0) + ascent,
        underline: (
            -px(metrics.underline_position()),
            px(metrics.underline_thickness()).max(1.0),
        ),
        strike: px(metrics.strikethrough_position()),
    })
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
