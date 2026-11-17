//! The terminal's look: the sixteen colours for a dark ground and a light one, the text colour of
//! the window's token sheet (`--fg`, read from `frame.css` by the tests), the ground the colours
//! are read against and the cursor's colour, the selection (the accent at 40%, the text colour at
//! 18% in a window or pane not in use), the matches of a find (the accent over the ground at 25%,
//! the one shown at 60%; the system's grey in a window not in use), the scrollbar (the text colour
//! at 40%, 90% pointed at), and a least contrast of 4.5 for text a program coloured for another
//! ground, 7 when the person asks for more contrast.

use crate::surface::{Palette, Rgb};

/// The sixteen colours on a dark ground and on a light one.
const DARK: [u32; 16] = [
    0x484f58, 0xff7b72, 0x3fb950, 0xd29922, 0x58a6ff, 0xbc8cff, 0x39c5cf, 0xb1bac4, 0x8b949e,
    0xffa198, 0x56d364, 0xe3b341, 0x79c0ff, 0xd2a8ff, 0x56d4dd, 0xf0f6fc,
];
const LIGHT: [u32; 16] = [
    0x24292e, 0xd73a49, 0x1a7f37, 0x8a6500, 0x0366d6, 0x6f42c1, 0x1b7c83, 0x6a737d, 0x6a737d,
    0xcb2431, 0x22863a, 0x8a6500, 0x005cc5, 0x5a32a3, 0x1b7c83, 0x24292e,
];

/// The text colour (`--fg` of the token sheet), the ground the terminal's colours are read
/// against (it is never painted: the window's glass shows through) and the cursor, light and
/// dark.
pub(crate) const FG_LIGHT: Rgb = Rgb::hex(0x1d1d1f);
pub(crate) const FG_DARK: Rgb = Rgb::hex(0xe6e6e6);
const BG_LIGHT: Rgb = Rgb::hex(0xffffff);
const BG_DARK: Rgb = Rgb::hex(0x1e1e1e);
/// The accent of the token sheet (`--accent`), where the system names none.
pub(crate) const ACCENT: Rgb = Rgb::hex(0x0a84ff);
/// The accent of a window not in use, as the token sheet has it: the system's grey.
pub(crate) const GREY: Rgb = Rgb::hex(0x8e8e93);

/// The least contrast text keeps with its ground, and under Increase Contrast.
const CONTRAST: f64 = 4.5;
const CONTRAST_MORE: f64 = 7.0;

/// A colour and how much of it covers what is under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Paint {
    pub(crate) rgb: Rgb,
    /// 0 is clear, 255 opaque.
    pub(crate) alpha: u8,
}

impl Paint {
    pub(crate) const fn solid(rgb: Rgb) -> Self {
        Self { rgb, alpha: 255 }
    }

    /// `rgb` at `share` of full cover (0 to 1).
    pub(crate) fn share(rgb: Rgb, share: f64) -> Self {
        Self {
            rgb,
            alpha: channel(share * 255.0),
        }
    }
}

/// The look a terminal is drawn in: dark or light, the system's accent, whether its pane has
/// the keys in the key window (`active`) and whether its window is the key one (`key`), and
/// whether the person asked for more contrast.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each the system's or the person's own state, which the page was told the same way"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Theme {
    pub(crate) dark: bool,
    pub(crate) accent: Rgb,
    pub(crate) active: bool,
    pub(crate) key: bool,
    pub(crate) more: bool,
}

impl Theme {
    pub(crate) const fn new(dark: bool) -> Self {
        Self {
            dark,
            accent: ACCENT,
            active: true,
            key: true,
            more: false,
        }
    }

    /// The accent as it is drawn: the system's in the key window, its grey in another.
    pub(crate) const fn accent(self) -> Rgb {
        if self.key { self.accent } else { GREY }
    }

    pub(crate) const fn fg(self) -> Rgb {
        if self.dark { FG_DARK } else { FG_LIGHT }
    }

    pub(crate) const fn bg(self) -> Rgb {
        if self.dark { BG_DARK } else { BG_LIGHT }
    }

    /// The colours given to the terminal: what programs name is answered from them.
    pub(crate) fn palette(self) -> Palette {
        let table = if self.dark { DARK } else { LIGHT };
        Palette {
            foreground: self.fg(),
            background: self.bg(),
            cursor: self.fg(),
            ansi: table.map(Rgb::hex),
        }
    }

    pub(crate) fn selection(self) -> Paint {
        match self.active {
            true => Paint::share(self.accent, 0.4),
            false => Paint::share(self.fg(), 0.18),
        }
    }

    /// Every match of a find, and the one shown (with the accent as its edge).
    pub(crate) fn found(self, shown: bool) -> Paint {
        let share = if shown { 0.6 } else { 0.25 };
        Paint::solid(mix(self.accent(), self.bg(), share))
    }

    pub(crate) fn scrollbar(self, pointed: bool) -> Paint {
        let alpha = if pointed { 0xe6 } else { 0x66 };
        Paint {
            rgb: self.fg(),
            alpha,
        }
    }

    /// The least contrast text keeps with its ground in this look.
    pub(crate) const fn contrast(self) -> f64 {
        if self.more { CONTRAST_MORE } else { CONTRAST }
    }

    /// `fg` as drawn on `bg`: as it is when it reads well enough, else lightened or darkened,
    /// whichever reads better, until it does.
    pub(crate) fn readable(self, fg: Rgb, bg: Rgb) -> Rgb {
        let (ground, least) = (luminance(bg), self.contrast());
        if ratio(ground, luminance(fg)) >= least {
            return fg;
        }
        let toward = |up: bool| shifted(fg, ground, up, least);
        let (first, second) = match luminance(fg) < ground {
            true => (toward(false), toward(true)),
            false => (toward(true), toward(false)),
        };
        let first_ratio = ratio(ground, luminance(first));
        match first_ratio >= least || first_ratio > ratio(ground, luminance(second)) {
            true => first,
            false => second,
        }
    }
}

/// `a` over `b` at `share`.
fn mix(a: Rgb, b: Rgb, share: f64) -> Rgb {
    let one = |a: u8, b: u8| channel(f64::from(a) * share + f64::from(b) * (1.0 - share));
    Rgb {
        r: one(a.r, b.r),
        g: one(a.g, b.g),
        b: one(a.b, b.b),
    }
}

/// A channel from a value 0 to 255, rounded and held in range.
fn channel(value: f64) -> u8 {
    // The value is held to 0..=255 first, so the cast keeps it whole.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a byte's range just before"
    )]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

/// `fg` stepped a tenth of the way to white (`up`) or black each time until it reads on a
/// ground of luminance `ground` at `least`, or can go no further.
fn shifted(fg: Rgb, ground: f64, up: bool, least: f64) -> Rgb {
    let step = |c: u8| -> u8 {
        let c = f64::from(c);
        match up {
            true => channel(c + ((255.0 - c) * 0.1).ceil()),
            false => channel(c - (c * 0.1).ceil()),
        }
    };
    let mut colour = fg;
    let end = if up { 255 } else { 0 };
    while ratio(ground, luminance(colour)) < least && [colour.r, colour.g, colour.b] != [end; 3] {
        colour = Rgb {
            r: step(colour.r),
            g: step(colour.g),
            b: step(colour.b),
        };
    }
    colour
}

/// WCAG relative luminance.
fn luminance(colour: Rgb) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(colour.r) + 0.7152 * linear(colour.g) + 0.0722 * linear(colour.b)
}

/// WCAG contrast ratio of two luminances.
fn ratio(a: f64, b: f64) -> f64 {
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
