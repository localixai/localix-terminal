//! The terminal's fonts on macOS: the person's code font, read from its stack of names (the
//! Settings' CSS stack: the system's monospace for `ui-monospace` and `SFMono-Regular`, Menlo for
//! `monospace`, else the first family that is installed), at its size, in its four faces, and
//! the cell its characters fill: the advance of a `W` wide and the font's line tall, as a
//! terminal page measured it. With ligatures off each face is made with its joining features
//! off (`liga`, and `calt`, where fonts such as Fira Code and JetBrains Mono keep their arrows).

use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::CGGlyph;
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontOrientation, CTFontSymbolicTraits,
    kCTFontFeatureSettingsAttribute, kCTFontOpenTypeFeatureTag, kCTFontOpenTypeFeatureValue,
};

use super::super::draw::{Faces, Metrics};

/// One font in its four faces, and its measures in points.
pub(crate) struct Fonts {
    regular: CFRetained<CTFont>,
    bold: CFRetained<CTFont>,
    italic: CFRetained<CTFont>,
    bold_italic: CFRetained<CTFont>,
    /// A cell's width and height.
    pub(super) cell: (f64, f64),
    /// From a cell's top to the baseline.
    pub(super) baseline: f64,
    /// Under the baseline, and how thick an underline is.
    pub(super) underline: (f64, f64),
    /// Half the height of a lower-case letter: where a strike goes.
    pub(super) strike: f64,
    /// Whether its letters join as the font has them.
    pub(super) ligatures: bool,
}

// SAFETY: Core Text fonts are immutable and may be used from any thread.
unsafe impl Send for Fonts {}

impl Fonts {
    /// The first font of `stack` that is installed, at `size` points; Menlo where none is; its
    /// ligatures joining or not.
    pub(super) fn new(stack: &str, size: f64, ligatures: bool) -> Option<Self> {
        let found = stack
            .split(',')
            .find_map(|name| named(name, size))
            .or_else(|| named("Menlo", size))?;
        let regular = if ligatures { found } else { unjoined(&found) };
        let face = |traits: CTFontSymbolicTraits| {
            // SAFETY: a copy of a live font with the traits asked for (its features kept); none
            // when the family has no such face, and the regular face is drawn instead.
            unsafe { regular.copy_with_symbolic_traits(0.0, std::ptr::null(), traits, traits) }
                .unwrap_or_else(|| regular.clone())
        };
        let bold = face(CTFontSymbolicTraits::BoldTrait);
        let italic = face(CTFontSymbolicTraits::ItalicTrait);
        let bold_italic = face(CTFontSymbolicTraits::BoldTrait | CTFontSymbolicTraits::ItalicTrait);
        // SAFETY: the measures of a live font (this and the next five).
        let ascent = unsafe { regular.ascent() };
        // SAFETY: as above.
        let descent = unsafe { regular.descent() };
        // SAFETY: as above.
        let leading = unsafe { regular.leading() };
        let height = (ascent + descent + leading).ceil();
        let width = advance(&regular, 'W').unwrap_or(size * 0.6);
        // SAFETY: as above.
        let under = unsafe { regular.underline_position() };
        // SAFETY: as above.
        let thickness = unsafe { regular.underline_thickness() };
        let underline = (under, thickness);
        // SAFETY: as above.
        let x_height = unsafe { regular.x_height() };
        Some(Self {
            cell: (width, height),
            baseline: ((height - ascent - descent) / 2.0) + ascent,
            underline: (-underline.0, underline.1.max(1.0)),
            strike: x_height / 2.0,
            ligatures,
            regular,
            bold,
            italic,
            bold_italic,
        })
    }

    /// The face for bold and italic text.
    pub(super) fn face(&self, bold: bool, italic: bool) -> &CTFont {
        match (bold, italic) {
            (false, false) => &self.regular,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (true, true) => &self.bold_italic,
        }
    }
}

impl Faces for Fonts {
    fn metrics(&self) -> Metrics {
        Metrics {
            cell: self.cell,
            baseline: self.baseline,
            underline: self.underline,
            strike: self.strike,
        }
    }
}

/// `font` with the features that join letters off: standard ligatures and contextual
/// alternates.
fn unjoined(font: &CTFont) -> CFRetained<CTFont> {
    let off = CFNumber::new_i32(0);
    let feature = |tag: &str| {
        let tag = CFString::from_str(tag);
        // SAFETY: Core Text's constants, alive for the process.
        let keys = unsafe { [kCTFontOpenTypeFeatureTag, kCTFontOpenTypeFeatureValue] };
        let values: [&CFType; 2] = [tag.as_ref(), off.as_ref()];
        CFDictionary::<CFString, CFType>::from_slices(&keys, &values)
    };
    let (liga, calt) = (feature("liga"), feature("calt"));
    let features = CFArray::from_objects(&[&*liga, &*calt]);
    // SAFETY: Core Text's constant, alive for the process.
    let key = unsafe { kCTFontFeatureSettingsAttribute };
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&[key], &[features.as_ref()]);
    // SAFETY: a descriptor of a dictionary made just now.
    let descriptor = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };
    // SAFETY: a copy of a live font at its own size (0), with the descriptor's features.
    unsafe { font.copy_with_attributes(0.0, std::ptr::null(), Some(&descriptor)) }
}

/// The font `name` names (a family, or one of the stack's generic names) at `size`, when it is
/// installed.
fn named(name: &str, size: f64) -> Option<CFRetained<CTFont>> {
    let name = name.trim().trim_matches(['"', '\'']);
    match name {
        "ui-monospace" | "SFMono-Regular" => system_monospace(size),
        "monospace" => named("Menlo", size),
        "" => None,
        family => {
            let wanted = CFString::from_str(family);
            // SAFETY: a font made by its name; Core Text gives a stand-in when none has it.
            let font = unsafe { CTFont::with_name(&wanted, size, std::ptr::null()) };
            // SAFETY: the family of a live font.
            let found = unsafe { font.family_name() }.to_string();
            found.eq_ignore_ascii_case(family).then_some(font)
        }
    }
}

/// The system's own monospaced font, as `ui-monospace` is in a page.
fn system_monospace(size: f64) -> Option<CFRetained<CTFont>> {
    let weight_regular = 0.0_f64;
    // SAFETY: a class method of AppKit's font, which returns a font or nil.
    let font: Option<Retained<AnyObject>> = unsafe {
        msg_send![class!(NSFont), monospacedSystemFontOfSize: size, weight: weight_regular]
    };
    let raw = NonNull::new(Retained::into_raw(font?))?;
    // SAFETY: AppKit's font is Core Text's, the same object (toll-free bridged); the one retain
    // `Retained` held is handed over.
    Some(unsafe { CFRetained::from_raw(raw.cast::<CTFont>()) })
}

/// How far `character` advances in `font`, when the font has it.
fn advance(font: &CTFont, character: char) -> Option<f64> {
    let mut units = [0u16; 2];
    let units = character.encode_utf16(&mut units);
    let mut glyph: CGGlyph = 0;
    let (chars, glyphs) = (NonNull::new(units.as_mut_ptr())?, NonNull::from(&mut glyph));
    // SAFETY: one character in, one glyph out, both alive through the call.
    let found = unsafe { font.glyphs_for_characters(chars, glyphs, 1) };
    let orientation = CTFontOrientation::Horizontal;
    // SAFETY: one glyph's advance, read with no buffer for it.
    let width = unsafe {
        font.advances_for_glyphs(orientation, NonNull::from(&glyph), std::ptr::null_mut(), 1)
    };
    found.then_some(width)
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
