//! The characters a program draws boxes and bars with (U+2500 to U+259F), drawn as lines and blocks
//! that fill their cell to its edges, so a box's sides meet across rows and columns whatever the
//! font's own glyphs are. A line is light (a point thick), heavy (two) or double; a dashed one is
//! drawn whole; diagonals are left to the font.

/// A rectangle in a cell, in pixels from its top left, and how much of the colour covers it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Piece {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) share: f64,
}

/// For each line character U+2500 to U+257F, its arms up, right, down and left: 0 none, 1 light,
/// 2 heavy, 3 double; 9 for one left to the font.
const ARMS: &[u8; 512] = b"\
010102021010202001010202101020200101020210102020011002100120022000110012002100221100120021002200\
100110022001200211101210211011202120221012202220101110122011102120212012102220220111011202110212\
012101220221022211011102120112022101210222012202111111121211121221111121212121122211112212212212\
122221222221222201010202101020200303303003100130033000130031003313003100330010033001300313103130\
333010133031303303130131033313033101330313133131333301100011100111009999999999990001100001000010\
00022000020000200201102001022010";

/// Whether `c` is drawn here rather than by the font.
pub(crate) fn drawn(c: char) -> bool {
    match u32::from(c) {
        0x2500..=0x257f => arms(c).is_some(),
        0x2580..=0x259f => true,
        _ => false,
    }
}

fn arms(c: char) -> Option<[u8; 4]> {
    let at = usize::try_from(u32::from(c).checked_sub(0x2500)?).ok()? * 4;
    let arms = ARMS.get(at..at + 4)?;
    let arms = [arms[0], arms[1], arms[2], arms[3]].map(|digit| digit - b'0');
    (!arms.contains(&9)).then_some(arms)
}

/// The pieces `c` is drawn as in a cell `width` by `height` pixels, a light line `line` thick.
pub(crate) fn pieces(c: char, width: f64, height: f64, line: f64) -> Vec<Piece> {
    let piece = |x: f64, y: f64, w: f64, h: f64| Piece {
        x,
        y,
        width: w,
        height: h,
        share: 1.0,
    };
    match u32::from(c) {
        0x2500..=0x257f => arms(c)
            .map(|arms| lines(arms, width, height, line))
            .unwrap_or_default(),
        code @ 0x2580..=0x259f => blocks(code, width, height, piece),
        _ => vec![],
    }
}

/// The lines of a character with these `arms`, each from the cell's edge to past its middle, so
/// they join.
fn lines(arms: [u8; 4], width: f64, height: f64, line: f64) -> Vec<Piece> {
    let thick = |weight: u8| match weight {
        2 => line * 2.0,
        3 => line * 3.0,
        _ => line,
    };
    let [up, right, down, left] = arms;
    let (cx, cy) = ((width / 2.0).floor(), (height / 2.0).floor());
    // How far a horizontal arm reaches past the middle: half the widest vertical one.
    let across = |a: u8, b: u8| thick(a.max(b)) / 2.0;
    let reach_x = if up | down > 0 { across(up, down) } else { 0.0 };
    let reach_y = if left | right > 0 {
        across(left, right)
    } else {
        0.0
    };
    let mut out = vec![];
    let mut arm = |weight: u8, horizontal: bool, from: f64, to: f64| {
        if weight == 0 {
            return;
        }
        let size = thick(weight);
        let middle = if horizontal { cy } else { cx };
        let strokes: &[f64] = if weight == 3 { &[-line, line] } else { &[0.0] };
        for offset in strokes {
            let along = (from.min(to), (to - from).abs());
            let stroke = if weight == 3 { line } else { size };
            let at = (middle + offset - stroke / 2.0).round();
            out.push(match horizontal {
                true => Piece {
                    x: along.0,
                    y: at,
                    width: along.1,
                    height: stroke,
                    share: 1.0,
                },
                false => Piece {
                    x: at,
                    y: along.0,
                    width: stroke,
                    height: along.1,
                    share: 1.0,
                },
            });
        }
    };
    arm(left, true, 0.0, cx + reach_x);
    arm(right, true, cx - reach_x, width);
    arm(up, false, 0.0, cy + reach_y);
    arm(down, false, cy - reach_y, height);
    out
}

/// The block characters: halves and eighths of the cell, its quarters, and three shades.
fn blocks(code: u32, w: f64, h: f64, piece: impl Fn(f64, f64, f64, f64) -> Piece) -> Vec<Piece> {
    let eighth = |n: u32| f64::from(n) / 8.0;
    let (half_w, half_h) = ((w / 2.0).round(), (h / 2.0).round());
    let quarters = |bits: u8| {
        let mut out = vec![];
        for (bit, x, y) in [
            (1, 0.0, 0.0),
            (2, half_w, 0.0),
            (4, 0.0, half_h),
            (8, half_w, half_h),
        ] {
            if bits & bit != 0 {
                out.push(piece(
                    x,
                    y,
                    if x > 0.0 { w - half_w } else { half_w },
                    if y > 0.0 { h - half_h } else { half_h },
                ));
            }
        }
        out
    };
    let shade = |share: f64| {
        vec![Piece {
            share,
            ..piece(0.0, 0.0, w, h)
        }]
    };
    match code {
        0x2580 => vec![piece(0.0, 0.0, w, half_h)],
        0x2581..=0x2588 => {
            let tall = (h * eighth(code - 0x2580)).round();
            vec![piece(0.0, h - tall, w, tall)]
        }
        0x2589..=0x258f => vec![piece(0.0, 0.0, (w * eighth(0x2590 - code)).round(), h)],
        0x2590 => vec![piece(half_w, 0.0, w - half_w, h)],
        0x2591 => shade(0.25),
        0x2592 => shade(0.5),
        0x2593 => shade(0.75),
        0x2594 => vec![piece(0.0, 0.0, w, (h / 8.0).round().max(1.0))],
        0x2595 => {
            let narrow = (w / 8.0).round().max(1.0);
            vec![piece(w - narrow, 0.0, narrow, h)]
        }
        // ▖ ▗ ▘ ▙ ▚ ▛ ▜ ▝ ▞ ▟: which quarters each fills (1 top left, 2 top right, 4 bottom
        // left, 8 bottom right).
        0x2596..=0x259f => {
            let fills = [4, 8, 1, 13, 9, 7, 11, 2, 6, 14];
            let at = usize::try_from(code - 0x2596).unwrap_or(0);
            quarters(fills.get(at).copied().unwrap_or(0))
        }
        _ => vec![],
    }
}

#[cfg(test)]
#[path = "boxes_tests.rs"]
mod tests;
