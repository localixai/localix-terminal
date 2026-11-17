//! A run of a terminal's text drawn by Core Text: laid out as one line in its face (so a font's
//! ligatures join, the system's fallback fonts and emoji fill in what the face lacks), each glyph
//! then put at the column its character starts at, so the text keeps to the grid whatever the
//! fonts' own widths. An emoji (a glyph of a font in colour) is drawn as the one glyph Core Text
//! made of its sequence (people joined, a flag, a skin tone), in the middle of every cell the
//! sequence covers, made smaller where it would be taller than the row or wider than its cells.

use std::ptr::NonNull;

use objc2_core_foundation::{
    CFAttributedString, CFDictionary, CFIndex, CFNumber, CFRange, CFRetained, CFString, CFType,
    CGPoint, Type,
};
use objc2_core_graphics::{CGContext, CGGlyph};
use objc2_core_text::{
    CTFont, CTFontOrientation, CTFontSymbolicTraits, CTLine, CTRun, kCTFontAttributeName,
    kCTLigatureAttributeName,
};

use super::super::layout::Run;
use super::fonts::Fonts;

/// Draws `run` with its baseline `baseline` pixels down from the top of a context flipped so
/// that y grows down, in cells `cell_width` pixels wide (`at`), its ligatures on or off as its
/// fonts were made.
pub(super) fn draw(context: &CGContext, fonts: &Fonts, run: &Run, at: (f64, f64)) {
    let (cell_width, baseline) = at;
    let font = fonts.face(run.bold, run.italic);
    let Some(line) = line(font, &run.text, fonts.ligatures) else {
        return;
    };
    let columns = Columns {
        each: utf16_columns(run),
        end: run.end,
        cell: (cell_width, fonts.cell.1.round().max(1.0)),
        baseline: fonts.baseline,
    };
    let paint = run.paint;
    let channel = |c: u8| f64::from(c) / 255.0;
    CGContext::set_rgb_fill_color(
        Some(context),
        channel(paint.rgb.r),
        channel(paint.rgb.g),
        channel(paint.rgb.b),
        channel(paint.alpha),
    );
    // SAFETY: the runs of a line made just now, kept alive by it.
    let runs = unsafe { line.glyph_runs() };
    // SAFETY: the array holds the line's runs and nothing else.
    let runs = unsafe { CFRetained::cast_unchecked::<objc2_core_foundation::CFArray<CTRun>>(runs) };
    CGContext::save_g_state(Some(context));
    CGContext::translate_ctm(Some(context), 0.0, baseline);
    // Glyphs are drawn the right way up in a context whose y grows down.
    CGContext::scale_ctm(Some(context), 1.0, -1.0);
    for glyph_run in runs.iter() {
        let drawn_font = run_font(&glyph_run).unwrap_or_else(|| font.retain());
        glyphs(context, &drawn_font, &glyph_run, &columns);
    }
    CGContext::restore_g_state(Some(context));
}

/// `text` laid out in `font` as one line, with or without its ligatures.
pub(super) fn line(font: &CTFont, text: &str, ligatures: bool) -> Option<CFRetained<CTLine>> {
    let string = CFString::from_str(text);
    let ligature = CFNumber::new_i32(i32::from(ligatures));
    // SAFETY: the attribute names are Core Text's constants, alive for the process.
    let keys: [&CFString; 2] = unsafe { [kCTFontAttributeName, kCTLigatureAttributeName] };
    let values: [&CFType; 2] = [font.as_ref(), ligature.as_ref()];
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    // SAFETY: a string and a dictionary of attributes, both alive through the call.
    let attributed =
        unsafe { CFAttributedString::new(None, Some(&string), Some(attributes.as_opaque())) }?;
    // SAFETY: a line of a live attributed string.
    Some(unsafe { CTLine::with_attributed_string(&attributed) })
}

/// The font Core Text drew `run` in: the face asked for, or a fallback for what it lacks.
fn run_font(run: &CTRun) -> Option<CFRetained<CTFont>> {
    // SAFETY: the run's attributes, kept alive by the run.
    let attributes = unsafe { run.attributes() };
    // SAFETY: the attribute this reads is a font in every run Core Text lays out.
    let attributes =
        unsafe { CFRetained::cast_unchecked::<CFDictionary<CFString, CTFont>>(attributes) };
    // SAFETY: Core Text's constant, alive for the process.
    attributes.get(unsafe { kCTFontAttributeName })
}

/// The column each UTF-16 unit of `run`'s text is in (Core Text counts its string by them).
fn utf16_columns(run: &Run) -> Vec<u16> {
    let mut columns = Vec::with_capacity(run.text.len());
    let mut cluster = 0;
    for (at, character) in run.text.char_indices() {
        while let Some((start, _)) = run.columns.get(cluster + 1) {
            if (*start as usize) > at {
                break;
            }
            cluster += 1;
        }
        let column = run.columns.get(cluster).map_or(0, |(_, column)| *column);
        columns.extend(std::iter::repeat_n(column, character.len_utf16()));
    }
    columns
}

/// Where a run's glyphs go: the column of each UTF-16 unit of its text, the column after its
/// last cell, a cell's size in pixels and the baseline down from a cell's top.
struct Columns {
    each: Vec<u16>,
    end: u16,
    cell: (f64, f64),
    baseline: f64,
}

/// Draws one glyph run, each glyph at its character's column plus how far Core Text put it
/// from the first glyph of that column (a mark over a letter keeps its place); an emoji's in the
/// middle of the cells it covers.
fn glyphs(context: &CGContext, font: &CTFont, run: &CTRun, map: &Columns) {
    let (columns, cell_width) = (&map.each[..], map.cell.0);
    // SAFETY: the count of a live run's glyphs.
    let count = unsafe { run.glyph_count() };
    let Ok(len) = usize::try_from(count) else {
        return;
    };
    if len == 0 {
        return;
    }
    let all = CFRange {
        location: 0,
        length: count,
    };
    let mut glyphs: Vec<CGGlyph> = vec![0; len];
    let mut placed = vec![CGPoint { x: 0.0, y: 0.0 }; len];
    let mut indices: Vec<CFIndex> = vec![0; len];
    let (Some(g), Some(p), Some(i)) = (
        NonNull::new(glyphs.as_mut_ptr()),
        NonNull::new(placed.as_mut_ptr()),
        NonNull::new(indices.as_mut_ptr()),
    ) else {
        return;
    };
    // SAFETY: each buffer holds the run's glyph count of its type, the range covers that count
    // from the run's start, and nothing else reaches the buffers while the run writes them.
    unsafe { run.glyphs(all, g) };
    // SAFETY: as above.
    unsafe { run.positions(all, p) };
    // SAFETY: as above.
    unsafe { run.string_indices(all, i) };
    let at: Vec<u16> = (indices.iter())
        .map(|index| {
            (usize::try_from(*index).ok().and_then(|at| columns.get(at))).map_or(0, |c| *c)
        })
        .collect();
    // SAFETY: the traits of a live font.
    let colour = unsafe { font.symbolic_traits() }.contains(CTFontSymbolicTraits::ColorGlyphsTrait);
    if colour {
        return emoji(context, font, (&glyphs, &at), map);
    }
    let mut column_start: Option<(u16, f64)> = None;
    for (point, column) in placed.iter_mut().zip(at) {
        let start = match column_start {
            Some((at, x)) if at == column => x,
            _ => point.x,
        };
        column_start = Some((column, start));
        point.x = f64::from(column) * cell_width + (point.x - start);
        point.y = 0.0;
    }
    // The places were moved through a borrow of their own since: the pointers are taken again,
    // so none is used past a later borrow of what it points into.
    let (Some(g), Some(p)) = (
        NonNull::new(glyphs.as_mut_ptr()),
        NonNull::new(placed.as_mut_ptr()),
    ) else {
        return;
    };
    // SAFETY: the glyphs and their places, `len` of each, alive and only read through the call.
    unsafe { font.draw_glyphs(g, p, len, context) };
}

/// Draws emoji glyphs, each alone, in the middle of the cells from its column to the next
/// glyph's (or the run's end), at most the row's height tall and its cells' width wide.
fn emoji(context: &CGContext, font: &CTFont, (glyphs, at): (&[CGGlyph], &[u16]), map: &Columns) {
    let (width, height) = map.cell;
    // SAFETY: the measures of a live font (this and the next two).
    let ascent = unsafe { font.ascent() };
    // SAFETY: as above.
    let descent = unsafe { font.descent() };
    // SAFETY: as above.
    let size = unsafe { font.size() };
    for (k, (glyph, column)) in glyphs.iter().zip(at).enumerate() {
        let end = (at.get(k + 1..).unwrap_or_default().iter())
            .find(|next| *next > column)
            .copied()
            .unwrap_or(map.end)
            .max(*column + 1);
        let span = f64::from(end - column) * width;
        let mut glyph = *glyph;
        let one = NonNull::from(&mut glyph);
        // SAFETY: one glyph's advance, read with no buffer for it.
        let advance = unsafe {
            font.advances_for_glyphs(CTFontOrientation::Horizontal, one, std::ptr::null_mut(), 1)
        };
        if advance <= 0.0 {
            continue;
        }
        let scale = (height / (ascent + descent)).min(span / advance).min(1.0);
        // SAFETY: a copy of a live font at another size, its own matrix and attributes kept.
        let drawn = unsafe { font.copy_with_attributes(size * scale, std::ptr::null(), None) };
        let top = (height - (ascent + descent) * scale) / 2.0;
        let mut point = CGPoint {
            x: f64::from(*column) * width + (span - advance * scale) / 2.0,
            y: map.baseline - (top + ascent * scale),
        };
        // SAFETY: one glyph and its place, alive through the call.
        unsafe { drawn.draw_glyphs(one, NonNull::from(&mut point), 1, context) };
    }
}
