//! Workloads of text that is hard to place: characters two cells wide, emoji joined into one or
//! asked to show as pictures, letters with marks that combine with them; and lines far longer
//! than the screen is wide.

use super::{Rng, WORDS};

/// Pieces of text that take more than one code point or more than one cell each.
const PIECES: [&str; 18] = [
    // CJK, two cells each.
    "漢字",
    "終端機",
    "テスト",
    "한국어",
    // Emoji: one, joined by ZWJ, with a skin tone, asked as a picture by VS16, a flag.
    "😀",
    "👨\u{200d}👩\u{200d}👧\u{200d}👦",
    "🏳\u{fe0f}\u{200d}🌈",
    "👍🏽",
    "❤\u{fe0f}",
    "✔\u{fe0f}",
    "🇯🇵",
    // Letters with combining marks, one and several.
    "e\u{301}",
    "a\u{308}\u{323}",
    "n\u{303}",
    "o\u{31b}\u{301}",
    // Precomposed letters and other scripts of one cell.
    "naïve",
    "Привет",
    "Ελληνικά",
];

/// Lines that mix plain words with the pieces above, each at most about 100 cells, up to `size`.
pub(super) fn unicode(size: usize) -> Vec<u8> {
    let mut rng = Rng::new(0x0dec0de);
    let mut out = String::with_capacity(size + 256);
    while out.len() < size {
        let mut cells = 0;
        while cells < 90 {
            let piece = match rng.below(3) {
                0 => *rng.pick(&WORDS),
                _ => *rng.pick(&PIECES),
            };
            out.push_str(piece);
            out.push(' ');
            // Wide enough to stay inside the row: each piece takes at most twice its characters.
            cells += 2 * piece.chars().count() + 1;
        }
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// Lines of 64 KiB with no line break, each wrapped by the terminal over 546 rows of 120, then
/// ended; words of plain text with a coloured one now and then.
pub(super) fn long_line(size: usize) -> Vec<u8> {
    const LINE: usize = 64 << 10;
    let mut rng = Rng::new(0x1046);
    let mut out = String::with_capacity(size + LINE);
    while out.len() < size {
        let start = out.len();
        while out.len() - start < LINE {
            match rng.below(10) {
                0 => {
                    out.push_str("\x1b[1;32m");
                    out.push_str(rng.pick(&WORDS));
                    out.push_str("\x1b[0m");
                }
                _ => out.push_str(rng.pick(&WORDS)),
            }
            out.push(' ');
        }
        out.push_str("\r\n");
    }
    out.into_bytes()
}
