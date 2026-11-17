//! A run of a terminal's text drawn by Pango into Cairo: laid out as one line in its face (so a
//! font's ligatures join, and fontconfig's fallback fonts and emoji fill in what the face
//! lacks), each glyph then put at the column its character starts at, so the text keeps to the
//! grid whatever the fonts' own widths. An emoji (a glyph of a colour emoji font) is drawn in
//! the middle of the cells its sequence covers, made smaller where it would be taller than the
//! row or wider than its cells.

use gtk::cairo;
use gtk::pango::{self, prelude::FontExt};

use super::super::layout::Run;
use super::super::theme::Paint;
use super::fonts::{Fonts, UNITS};

/// Draws `run` with the grid's left and its baseline at `at`, in cells `cell_width` pixels wide.
pub(super) fn draw(cr: &cairo::Context, fonts: &Fonts, run: &Run, at: (f64, f64), cell_width: f64) {
    let layout = pango::Layout::new(&fonts.context);
    layout.set_font_description(Some(fonts.face(run.bold, run.italic)));
    layout.set_attributes(Some(&fonts.features));
    layout.set_text(&run.text);
    let Some(line) = layout.line_readonly(0) else {
        return;
    };
    paint(cr, run.paint);
    let cell = (cell_width, fonts_row(fonts));
    for item in line.runs() {
        let offset = usize::try_from(item.item().offset()).unwrap_or(0);
        let font = item.item().analysis().font();
        let mut glyphs = item.glyph_string();
        let columns: Vec<u16> = (glyphs.log_clusters().iter())
            .map(|at| column_of(run, offset + usize::try_from(*at).unwrap_or(0)))
            .collect();
        let emoji = font
            .describe()
            .family()
            .is_some_and(|family| family.to_lowercase().contains("emoji"));
        let place = Place {
            columns: &columns,
            end: run.end,
            cell,
            at,
        };
        match emoji {
            true => emojis(cr, fonts, &font, &mut glyphs, &place),
            false => pinned(cr, &font, &mut glyphs, &place),
        }
    }
}

/// A row's height in pixels, as the grid has it.
fn fonts_row(fonts: &Fonts) -> f64 {
    use super::super::draw::Faces;
    fonts.metrics().cell.1.round().max(1.0)
}

fn paint(cr: &cairo::Context, paint: Paint) {
    let part = |c: u8| f64::from(c) / 255.0;
    let Paint { rgb, alpha } = paint;
    cr.set_source_rgba(part(rgb.r), part(rgb.g), part(rgb.b), part(alpha));
}

/// The column the character at byte `at` of `run`'s text is in.
fn column_of(run: &Run, at: usize) -> u16 {
    (run.columns.iter())
        .take_while(|(start, _)| *start as usize <= at)
        .last()
        .map_or(0, |(_, column)| *column)
}

/// Where a glyph string's glyphs go: each one's column, the column after the run's last cell,
/// a cell's size in pixels, and the grid's left and the baseline.
struct Place<'a> {
    columns: &'a [u16],
    end: u16,
    cell: (f64, f64),
    at: (f64, f64),
}

/// Draws `glyphs`, each glyph at its character's column plus how far Pango put it from the
/// first glyph of that column (a mark over a letter keeps its place).
fn pinned(
    cr: &cairo::Context,
    font: &pango::Font,
    glyphs: &mut pango::GlyphString,
    place: &Place<'_>,
) {
    let mut natural = 0;
    let mut column_start: Option<(u16, i32)> = None;
    for (info, column) in glyphs.glyph_info_mut().iter_mut().zip(place.columns) {
        let geometry = info.geometry_mut();
        let start = match column_start {
            Some((at, x)) if at == *column => x,
            _ => natural,
        };
        column_start = Some((*column, start));
        let target = f64::from(*column) * place.cell.0 * UNITS;
        let x = whole(target) + (natural - start) + geometry.x_offset();
        natural += geometry.width();
        geometry.set_width(0);
        geometry.set_x_offset(x);
    }
    cr.move_to(place.at.0, place.at.1);
    pangocairo::functions::show_glyph_string(cr, font, glyphs);
}

/// Draws emoji glyphs in the middle of the cells from each one's column to the next's (or the
/// run's end), at most the row's height tall and their cells' width wide.
fn emojis(
    cr: &cairo::Context,
    fonts: &Fonts,
    font: &pango::Font,
    glyphs: &mut pango::GlyphString,
    place: &Place<'_>,
) {
    use super::super::draw::Faces;
    let metrics = font.metrics(None);
    let tall = f64::from(metrics.ascent() + metrics.descent()) / UNITS;
    let (width, height) = place.cell;
    let row_top = place.at.1 - fonts.metrics().baseline;
    let ends: Vec<u16> = (0..place.columns.len())
        .map(|k| {
            let column = place.columns.get(k).copied().unwrap_or(0);
            (place.columns.get(k + 1..).unwrap_or_default().iter())
                .find(|next| **next > column)
                .copied()
                .unwrap_or(place.end)
                .max(column + 1)
        })
        .collect();
    let advances: Vec<f64> = (glyphs.glyph_info().iter())
        .map(|info| f64::from(info.geometry().width()) / UNITS)
        .collect();
    let scale = (place.columns.iter().zip(&ends).zip(&advances))
        .filter(|(_, advance)| **advance > 0.0)
        .map(|((column, end), advance)| (f64::from(end - column) * width) / advance)
        .fold((height / tall.max(1.0)).min(1.0), f64::min);
    for (k, info) in glyphs.glyph_info_mut().iter_mut().enumerate() {
        let geometry = info.geometry_mut();
        let column = place.columns.get(k).copied().unwrap_or(0);
        let span = f64::from(ends.get(k).copied().unwrap_or(column + 1) - column) * width;
        let advance = advances.get(k).copied().unwrap_or(0.0);
        let left = f64::from(column) * width + (span - advance * scale) / 2.0;
        // In the scaled space the glyphs are drawn in, from where the string starts.
        let x = whole(left / scale * UNITS) + geometry.x_offset();
        geometry.set_width(0);
        geometry.set_x_offset(x);
    }
    let ascent = f64::from(metrics.ascent()) / UNITS;
    let baseline = row_top + (height - tall * scale) / 2.0 + ascent * scale;
    let _ = cr.save();
    cr.translate(place.at.0, baseline);
    cr.scale(scale, scale);
    cr.move_to(0.0, 0.0);
    pangocairo::functions::show_glyph_string(cr, font, glyphs);
    let _ = cr.restore();
}

/// `n` Pango units, whole.
fn whole(n: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a place in a row, in Pango units, held to an i32's range just before"
    )]
    let n = n.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    n
}
