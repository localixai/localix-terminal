//! The terminal canvas on macOS, drawn without a window and held to pictures kept beside the
//! tests (`tests/fixtures/view/macos/*.png`): recordings of real sessions (boxes, wide
//! characters, a full-screen program, a pager's colours) at 2× in the default font, dark and
//! light, each within 0.5% of its pixels and a colour difference of 3 (CIE76) of the one kept;
//! and the terminal's own ground left clear, so the window's glass shows through.
//! `LLXTERM_BLESS=1` writes the pictures anew, to be looked at before they are kept.

#![cfg(target_os = "macos")]
// Clippy 1.100 reads a test crate's helpers as test code, where clippy.toml lets a test unwrap,
// panic and print; clippy before it reads only a `#[test]` function so. An `expect` of these
// holds under one and fails under the other, so they are allowed.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic fails the test"
)]

#[path = "support/cast.rs"]
mod cast;
#[path = "support/png.rs"]
mod png;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use cast::{Cast, Event};
use llxterm::browser::{Picture, TermLook, term_picture};
use llxterm::surface::{CellPixels, Frame};
use llxterm::terminal::{Core, Render};

fn fixture(dir: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir)
        .join(name)
}

/// A core of `cast` played up to its event `last`, in `look`'s colours.
fn core_of(cast: &Cast, last: usize, look: &TermLook) -> Core {
    let mut core = Core::new(cast.rows, cast.columns).unwrap();
    core.set_colors(&look.palette());
    for (_, event) in cast.events.iter().take(last + 1) {
        match event {
            Event::Output(text) => core.feed(text.as_bytes()),
            Event::Resize(rows, columns) => {
                core.resize(*rows, *columns, CellPixels::default()).unwrap()
            }
        }
    }
    core
}

fn frame_of(core: &Core) -> Frame {
    let mut frame = Frame::default();
    assert!(Render::new().unwrap().frame(core, &mut frame, true));
    frame
}

/// The frame a recording leaves, in `look`'s colours; for a full-screen program, the last frame
/// it drew before it let the screen go.
fn played(name: &str, look: &TermLook) -> Frame {
    let cast = Cast::read(&fixture("terminal", &format!("{name}.cast")));
    let mut last = cast.events.len() - 1;
    let mut core = core_of(&cast, last, look);
    if ["vim", "top"].contains(&name) {
        let mut probe = Core::new(cast.rows, cast.columns).unwrap();
        for (at, (_, event)) in cast.events.iter().enumerate() {
            if let Event::Output(text) = event {
                probe.feed(text.as_bytes());
            }
            if probe.alternate() {
                last = at;
            }
        }
        core = core_of(&cast, last, look);
    }
    frame_of(&core)
}

/// Every colour, attribute and line a program can set, ligatures, a selection and the matches
/// of a find.
fn styles(look: &TermLook) -> Frame {
    let mut core = Core::new(14, 72).unwrap();
    core.set_colors(&look.palette());
    let mut out = String::new();
    for base in [30, 90] {
        for n in 0..8 {
            let _ = write!(out, "\x1b[{}m colour{} \x1b[0m", base + n, n);
        }
        out.push_str("\r\n");
    }
    for n in 0..8 {
        let _ = write!(out, "\x1b[{}m  {}  \x1b[0m", 40 + n, n);
    }
    out.push_str("\r\n");
    for n in (16..232).step_by(6) {
        let _ = write!(out, "\x1b[48;5;{n}m \x1b[0m");
    }
    out.push_str("\r\n");
    for n in 0..72 {
        let _ = write!(out, "\x1b[48;2;{};{};200m \x1b[0m", n * 3, 255 - n * 3);
    }
    out.push_str(
        "\r\n\x1b[1mbold\x1b[0m \x1b[3mitalic\x1b[0m \x1b[1;3mboth\x1b[0m \x1b[2mfaint\x1b[0m ",
    );
    out.push_str(
        "\x1b[7minverse\x1b[0m \x1b[9mstrike\x1b[0m \x1b[53mover\x1b[0m \x1b[8mhidden\x1b[0m\r\n",
    );
    out.push_str("\x1b[4msingle\x1b[0m \x1b[4:2mdouble\x1b[0m \x1b[4:3mcurly\x1b[0m ");
    out.push_str("\x1b[4:4mdotted\x1b[0m \x1b[4:5mdashed\x1b[0m \x1b[4;58;5;1mred line\x1b[0m\r\n");
    out.push_str("ligatures: -> => != === <= >= :: <!-- --> www\r\n");
    out.push_str("wide: 日本語 한국어 emoji 👍 ❤️\r\n");
    out.push_str("box: ┌──┬──┐\r\n     │  │  │\r\n     └──┴──┘\r\n");
    out.push_str("find the needle and the needle");
    core.feed(out.as_bytes());
    let mut frame = frame_of(&core);
    frame.rows[12].selected = Some((0, 3));
    frame.matches = vec![
        llxterm::surface::Span {
            y: 12,
            from: 9,
            to: 14,
        },
        llxterm::surface::Span {
            y: 12,
            from: 24,
            to: 29,
        },
    ];
    frame.current = Some(frame.matches[1]);
    frame
}

/// CIE L*a*b* of an sRGB colour.
fn lab(rgb: [u8; 3]) -> [f64; 3] {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = rgb.map(linear);
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// How far two premultiplied pixels are apart, as seen over black (alpha counts as lightness).
fn delta(a: &[u8], b: &[u8]) -> f64 {
    let (la, lb) = (lab([a[0], a[1], a[2]]), lab([b[0], b[1], b[2]]));
    let colour = la
        .iter()
        .zip(lb)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt();
    colour.max((f64::from(a[3]) - f64::from(b[3])).abs() * 100.0 / 255.0)
}

/// `picture` held to the one kept as `name`, or kept anew under `LLXTERM_BLESS=1`.
fn holds(name: &str, picture: &Picture) {
    let path = fixture("view/macos", &format!("{name}.png"));
    let drawn = png::Pixels {
        width: picture.width,
        height: picture.height,
        rgba: picture.rgba.clone(),
    };
    if std::env::var_os("LLXTERM_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        png::write(&path, &drawn);
        return;
    }
    let kept = png::load(&path);
    assert_eq!(
        (kept.width, kept.height),
        (drawn.width, drawn.height),
        "{name}: its size"
    );
    let pixels = kept.rgba.len() / 4;
    let off = (kept
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(drawn.rgba.as_chunks::<4>().0.iter()))
    .filter(|(a, b)| delta(&a[..], &b[..]) > 3.0)
    .count();
    assert!(
        off * 200 <= pixels,
        "{name}: {off} of {pixels} pixels differ (LLXTERM_BLESS=1 to keep anew)"
    );
}

#[test]
fn recordings_draw_as_the_pictures_kept() {
    for (name, dark) in [
        ("boxes", true),
        ("boxes", false),
        ("wide", true),
        ("vim", true),
        ("top", true),
    ] {
        let look = TermLook::new(dark, 2.0);
        let picture = term_picture(&played(name, &look), &look).unwrap();
        let shade = if dark { "dark" } else { "light" };
        holds(&format!("{name}-{shade}"), &picture);
    }
}

#[test]
fn colours_attributes_lines_selection_and_matches_draw_as_kept() {
    for dark in [true, false] {
        // The light one as in a window not in use: the cursor an outline, the selection grey.
        let look = TermLook {
            active: dark,
            ..TermLook::new(dark, 2.0)
        };
        let picture = term_picture(&styles(&look), &look).unwrap();
        holds(if dark { "styles-dark" } else { "styles-light" }, &picture);
    }
}

#[test]
fn the_terminals_ground_is_left_clear() {
    let look = TermLook::new(true, 2.0);
    let mut core = Core::new(4, 20).unwrap();
    core.set_colors(&look.palette());
    core.feed(b"hello \x1b[44m  \x1b[0m world\r\n");
    let mut frame = Frame::default();
    Render::new().unwrap().frame(&core, &mut frame, true);
    let picture = term_picture(&frame, &look).unwrap();
    let alpha = |x: usize, y: usize| picture.rgba[(y * picture.width + x) * 4 + 3];
    // The gutter, a blank cell after the text, and the empty rows are clear.
    assert_eq!(alpha(4, 4), 0, "the gutter");
    assert_eq!(alpha(picture.width - 40, 30), 0, "a blank cell");
    assert_eq!(
        alpha(picture.width / 2, picture.height - 30),
        0,
        "an empty row"
    );
    // A ground the program set is painted, opaque.
    let cell = picture
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] == 255 && p[2] > p[0])
        .count();
    assert!(cell > 0, "the blue ground is drawn");
    assert!(
        picture
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] == 255 && p[0] > 200),
        "the text is drawn"
    );
}

/// Where `text`, printed on the middle of three rows, draws in colour: the leftmost and the
/// rightmost of those pixels, the highest and the lowest, and the picture's height.
fn coloured(text: &str) -> ([usize; 4], usize) {
    let look = TermLook::new(true, 2.0);
    let mut core = Core::new(3, 20).unwrap();
    core.set_colors(&look.palette());
    core.feed(format!("\r\n{text}").as_bytes());
    let picture = term_picture(&frame_of(&core), &look).unwrap();
    let mut box_ = [usize::MAX, 0, usize::MAX, 0];
    for (at, p) in picture.rgba.as_chunks::<4>().0.iter().enumerate() {
        let (most, least) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
        if p[3] > 128 && most - least > 60 {
            let (x, y) = (at % picture.width, at / picture.width);
            box_ = [
                box_[0].min(x),
                box_[1].max(x),
                box_[2].min(y),
                box_[3].max(y),
            ];
        }
    }
    (box_, picture.height)
}

#[test]
fn an_emoji_sequence_is_one_glyph_across_its_cells_no_taller_than_its_row() {
    // Three people apart, and the same three joined into one family: the family is one
    // picture, in the two cells Unicode gives it, where the three apart begin.
    let (apart, height) = coloured("👨👩👧");
    let (family, _) = coloured("👨\u{200d}👩\u{200d}👧");
    let width = |b: [usize; 4]| b[1] - b[0];
    assert!(
        width(family) * 2 < width(apart),
        "joined {family:?}, apart {apart:?}"
    );
    assert!(
        family[0].abs_diff(apart[0]) < width(apart) / 6 && width(family) <= width(apart) / 3 + 2,
        "{family:?} {apart:?}"
    );
    // The middle row of three: an emoji drawn no taller than its row keeps off the other two.
    let row = height / 3;
    for drawn in [apart, family] {
        assert!(drawn[3] - drawn[2] <= row, "{drawn:?} in rows of {row}");
    }
    // A flag of two letters is one glyph too, as wide as its two cells at most.
    let (flag, _) = coloured("🇯🇵");
    assert!(width(flag) <= width(apart) / 2, "{flag:?}");
}
