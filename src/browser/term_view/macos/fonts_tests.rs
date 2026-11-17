//! Tests of `fonts`: with ligatures off, each face is made so letters its font joins are drawn
//! apart even where a line lets ligatures join; on, they join as the font has them. A font of the
//! system's with standard ligatures (`fi`) stands in for a code font's (`->`), which the system
//! has none of.

use super::super::text::line;
use super::Fonts;

/// How many glyphs `fonts` draw `text` in, in each face, with the line's ligatures let join.
fn glyphs(fonts: &Fonts, text: &str) -> Vec<isize> {
    [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .map(|(bold, italic)| {
            let made = line(fonts.face(bold, italic), text, true).expect("a line");
            // SAFETY: the glyphs of a live line.
            unsafe { made.glyph_count() }
        })
        .collect()
}

#[test]
fn ligatures_off_draw_the_letters_a_font_joins_apart_in_every_face() {
    let joined = Fonts::new("Hoefler Text", 26.0, true).expect("Hoefler Text is the system's");
    assert_eq!(
        glyphs(&joined, "fi"),
        [1, 1, 1, 1],
        "joined as the font has them"
    );
    let apart = Fonts::new("Hoefler Text", 26.0, false).expect("Hoefler Text is the system's");
    assert_eq!(glyphs(&apart, "fi"), [2, 2, 2, 2]);
}
