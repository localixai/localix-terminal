//! The modes a program set that the screen's model (`vt100`) does not keep, read from its output
//! as `screen.rs` passes it on, and written again after the screen whenever a page is sent the
//! whole of it (a page loaded again, a panel that dropped output, a flood): focus reports
//! (1004), autowrap (7), reverse video (5), insert (4), the cursor's shape (DECSCUSR) and colour
//! (OSC 12), the scroll region (DECSTBM) and origin mode (6). What the model keeps (the
//! alternate screen, the keypad, the cursor keys, bracketed paste, the mouse) it writes itself.
//! The bytes are the program's, untrusted: only numbers, and a colour that reads as one, are
//! kept and written, never the program's own sequences.

use std::fmt::Write as _;

/// A mode kept as on or off: whether it is a private one (`CSI ?`), its number, the final byte
/// (`h` or `l`) that leaves it other than a terminal starts, and what sets it so again.
struct Flag {
    private: bool,
    number: u16,
    set: u8,
    again: &'static str,
}

const FLAGS: [Flag; 4] = [
    Flag {
        private: true,
        number: 1004,
        set: b'h',
        again: "\x1b[?1004h",
    },
    Flag {
        private: true,
        number: 7,
        set: b'l',
        again: "\x1b[?7l",
    },
    Flag {
        private: true,
        number: 5,
        set: b'h',
        again: "\x1b[?5h",
    },
    Flag {
        private: false,
        number: 4,
        set: b'h',
        again: "\x1b[4h",
    },
];

/// Origin mode's number: it is written after the region, whose top it counts the cursor from.
const ORIGIN: u16 = 6;
/// The longest cursor colour kept: a name or a hex value is far shorter.
const COLOUR_MOST: usize = 32;

#[derive(Debug, Default, Clone)]
pub(super) struct Modes {
    /// Which of `FLAGS` the program set other than a terminal starts, a bit each.
    flags: u8,
    origin: bool,
    /// The cursor's shape (DECSCUSR, 1 to 6); none: the page's own.
    cursor: Option<u16>,
    colour: Option<String>,
    /// The scroll region's first row and last (none: the screen's last), from 1; none: the
    /// whole screen.
    region: Option<(u16, Option<u16>)>,
}

impl Modes {
    /// A CSI the program wrote: `body` is what came between `ESC [` and `end`, its final byte.
    pub(super) fn csi(&mut self, body: &[u8], end: u8) {
        let (private, body) = match body.split_first() {
            Some((b'?', rest)) => (true, rest),
            Some((b'<'..=b'>', _)) => return,
            _ => (false, body),
        };
        let at = (body.iter())
            .position(|byte| (0x20..=0x2f).contains(byte))
            .unwrap_or(body.len());
        let (numbers, middle) = body.split_at(at);
        let numbers: Vec<Option<u16>> = numbers.split(|byte| *byte == b';').map(number).collect();
        match (private, middle, end) {
            (_, [], b'h' | b'l') => self.flipped(private, &numbers, end),
            (false, [b' '], b'q') => self.shaped(numbers.first().copied().flatten()),
            (false, [], b'r') => self.scrolled(&numbers),
            // A soft reset (DECSTR) puts the modes back, as the page does; the colour is no mode.
            (false, [b'!'], b'p') => {
                *self = Self {
                    colour: self.colour.take(),
                    ..Self::default()
                };
            }
            _ => {}
        }
    }

    fn flipped(&mut self, private: bool, numbers: &[Option<u16>], end: u8) {
        for &number in numbers.iter().flatten() {
            if private && number == ORIGIN {
                self.origin = end == b'h';
            }
            for (bit, flag) in FLAGS.iter().enumerate() {
                if flag.private == private && flag.number == number {
                    if end == flag.set {
                        self.flags |= 1 << bit;
                    } else {
                        self.flags &= !(1 << bit);
                    }
                }
            }
        }
    }

    /// DECSCUSR: with no number a blinking block, as the page reads it; 0 the page's own.
    fn shaped(&mut self, shape: Option<u16>) {
        match shape.unwrap_or(1) {
            0 => self.cursor = None,
            shape @ 1..=6 => self.cursor = Some(shape),
            _ => {}
        }
    }

    /// DECSTBM: a region that ends before it starts is ignored, as the page ignores it.
    fn scrolled(&mut self, numbers: &[Option<u16>]) {
        let given = |at: usize| numbers.get(at).copied().flatten().filter(|n| *n > 0);
        let (top, bottom) = (given(0).unwrap_or(1), given(1));
        match bottom {
            Some(bottom) if bottom <= top => {}
            None if top == 1 => self.region = None,
            _ => self.region = Some((top, bottom)),
        }
    }

    /// A string sequence the program wrote (`12;#ff8800`): whether it was of the cursor's colour
    /// (OSC 12 sets it, OSC 112 takes it back). A query or what is no colour changes nothing.
    pub(super) fn osc(&mut self, said: &str) -> bool {
        let (number, value) = said.split_once(';').unwrap_or((said, ""));
        match number {
            "12" => {
                if let Some(colour) = colour(value) {
                    self.colour = Some(colour);
                }
                true
            }
            "112" => {
                self.colour = None;
                true
            }
            _ => false,
        }
    }

    /// A full reset (RIS): as a terminal starts.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// The screen took another size: the page lets go of the region then, and so does this.
    pub(super) fn resized(&mut self) {
        self.region = None;
    }

    /// What sets the modes again after the screen is drawn, the cursor at `cursor` (row and
    /// column, from 1, from the screen's top): the region homes the cursor, so it is put back.
    pub(super) fn replayed(&self, cursor: (u16, u16)) -> Vec<u8> {
        let mut out = String::new();
        for (bit, flag) in FLAGS.iter().enumerate() {
            if self.flags & (1 << bit) != 0 {
                out.push_str(flag.again);
            }
        }
        if let Some(shape) = self.cursor {
            let _ = write!(out, "\x1b[{shape} q");
        }
        if let Some(colour) = &self.colour {
            let _ = write!(out, "\x1b]12;{colour}\x07");
        }
        match self.region {
            Some((top, Some(bottom))) => drop(write!(out, "\x1b[{top};{bottom}r")),
            Some((top, None)) => drop(write!(out, "\x1b[{top}r")),
            None => {}
        }
        if self.origin {
            let _ = write!(out, "\x1b[?{ORIGIN}h");
        }
        if self.region.is_some() || self.origin {
            let top = (self.region.filter(|_| self.origin)).map_or(1, |(top, _)| top);
            let (row, column) = cursor;
            let _ = write!(out, "\x1b[{};{column}H", row.saturating_sub(top) + 1);
        }
        out.into_bytes()
    }
}

/// A CSI's number: digits alone, else none (a default, or a sub-parameter's `:`).
fn number(digits: &[u8]) -> Option<u16> {
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// `value` when it reads as a colour: `#` and hex digits, `rgb:` and three hex parts, or a name
/// of letters and digits; at most `COLOUR_MOST` bytes.
fn colour(value: &str) -> Option<String> {
    let hex = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_hexdigit());
    let sharp =
        (value.strip_prefix('#')).is_some_and(|h| hex(h) && [3, 6, 9, 12].contains(&h.len()));
    let rgb = (value.strip_prefix("rgb:")).is_some_and(|rest| {
        let parts: Vec<&str> = rest.split('/').collect();
        parts.len() == 3 && parts.iter().all(|part| hex(part) && part.len() <= 4)
    });
    let named = (value.bytes().next()).is_some_and(|first| first.is_ascii_alphabetic())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric());
    (value.len() <= COLOUR_MOST && (sharp || rgb || named)).then(|| value.to_string())
}

#[cfg(test)]
#[path = "modes_tests.rs"]
mod tests;
