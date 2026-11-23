//! What the cores are fed: a program's output of one kind each, made here from fixed seeds, so
//! every run and every core gets the same bytes. Each is made up to its size and ends where a
//! line or a frame ends, never inside a sequence. The kinds that draw text are here; those of
//! wide and joined characters and of a long line are `text.rs`; a full-screen program's frames
//! and the app's recordings are `screens.rs`.

mod screens;
mod text;

use std::fmt::Write;

/// One workload: its name in the tables, what it is, and its bytes.
pub struct Workload {
    pub name: &'static str,
    pub about: &'static str,
    pub bytes: Vec<u8>,
}

/// A maker of a workload of about `size` bytes.
type Make = fn(usize) -> Vec<u8>;

/// Every workload's name, what it is and its maker, in the order the tables show them.
const ALL: [(&str, &str, Make); 8] = [
    ("ascii", "numbered lines, as `seq` prints", ascii),
    (
        "dense",
        "every cell its own colour: truecolor text on a 256-colour ground",
        dense,
    ),
    (
        "scroll-region",
        "lines written into a scroll region, scrolled both ways",
        scroll_region,
    ),
    (
        "cursor",
        "short words written all over the screen, the cursor moved each time",
        cursor,
    ),
    (
        "unicode",
        "CJK, emoji joined and with VS16, combining marks",
        text::unicode,
    ),
    (
        "tui",
        "full-screen frames on the other screen, each in a synchronized update",
        screens::tui,
    ),
    (
        "long-line",
        "lines of 64 KiB, each wrapped over 546 rows",
        text::long_line,
    ),
    (
        "recorded",
        "the app's recorded sessions, one after another",
        screens::recorded,
    ),
];

/// The workloads named in `names` (all when it is empty), each made to `size` bytes; the
/// recorded sessions only where the app's recordings are.
pub fn chosen(names: &[String], size: usize) -> impl Iterator<Item = Workload> + '_ {
    ALL.iter()
        .filter(move |(name, ..)| names.is_empty() || names.iter().any(|n| n == name))
        .filter_map(move |&(name, about, make)| {
            let bytes = make(size);
            (!bytes.is_empty()).then_some(Workload { name, about, bytes })
        })
}

/// A fixed sequence of numbers (xorshift64*), the same for a seed on every machine.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn number(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.number() % n.max(1) as u64) as usize
    }

    /// One of `items`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Words to write: plain ASCII of the lengths a program's output has.
const WORDS: [&str; 16] = [
    "fn",
    "let",
    "match",
    "return",
    "error:",
    "warning",
    "src/main.rs",
    "=>",
    "Ok(())",
    "0x1f",
    "compiling",
    "test",
    "passed",
    "{",
    "}",
    "--release",
];

/// Lines of `1`, `2`, … as a program prints them through a terminal (each line ended with
/// CR LF), up to `size` bytes.
fn ascii(size: usize) -> Vec<u8> {
    let mut out = String::with_capacity(size + 16);
    let mut n = 1u64;
    while out.len() < size {
        let _ = write!(out, "{n}\r\n");
        n += 1;
    }
    out.into_bytes()
}

/// Rows whose every cell has a colour of its own: a truecolor foreground and a 256-colour
/// ground, a printable character each, a row of the screen's width at a time.
fn dense(size: usize) -> Vec<u8> {
    let mut rng = Rng::new(0xde75e);
    let mut out = String::with_capacity(size + 4096);
    while out.len() < size {
        for _ in 0..crate::COLUMNS {
            let (r, g, b) = (rng.below(256), rng.below(256), rng.below(256));
            let ground = rng.below(256);
            let glyph = char::from(b'!' + (rng.below(94) as u8));
            let _ = write!(out, "\x1b[38;2;{r};{g};{b};48;5;{ground}m{glyph}");
        }
        out.push_str("\x1b[0m\r\n");
    }
    out.into_bytes()
}

/// A line of `words` from `rng`, at most `width` characters.
fn line(rng: &mut Rng, width: usize) -> String {
    let mut text = String::new();
    loop {
        let word = rng.pick(&WORDS);
        if text.len() + word.len() + 1 > width {
            return text;
        }
        text.push_str(word);
        text.push(' ');
    }
}

/// A scroll region of rows 5 to 35, lines written at its foot so it scrolls up; now and then a
/// reverse index at its head and a scroll down or up by a few rows, as a pager or a log pane
/// above a status line does.
fn scroll_region(size: usize) -> Vec<u8> {
    let mut rng = Rng::new(0x5c2011);
    let mut out = String::from("\x1b[2J\x1b[1;1Hstatus above\x1b[40;1Hstatus below\x1b[5;35r");
    out.push_str("\x1b[35;1H");
    while out.len() < size {
        match rng.below(16) {
            0 => out.push_str("\x1b[5;1H\x1bM"),
            1 => {
                let _ = write!(out, "\x1b[{}T", 1 + rng.below(4));
            }
            2 => {
                let _ = write!(out, "\x1b[{}S", 1 + rng.below(4));
            }
            _ => {}
        }
        let text = line(&mut rng, 110);
        let _ = write!(out, "\x1b[35;1H\r\n{text}\x1b[K");
    }
    out.push_str("\x1b[r\x1b[40;1H");
    out.into_bytes()
}

/// Short words written at places all over the screen: an absolute move, or a move up, down,
/// left or right by a few, then a word, and now and then the rest of the row erased.
fn cursor(size: usize) -> Vec<u8> {
    let mut rng = Rng::new(0xc0450);
    let mut out = String::from("\x1b[2J");
    while out.len() < size {
        match rng.below(6) {
            0 => {
                let _ = write!(out, "\x1b[{}A", 1 + rng.below(8));
            }
            1 => {
                let _ = write!(out, "\x1b[{}B", 1 + rng.below(8));
            }
            2 => {
                let _ = write!(out, "\x1b[{}C", 1 + rng.below(20));
            }
            3 => {
                let _ = write!(out, "\x1b[{}D", 1 + rng.below(20));
            }
            _ => {
                let (row, column) = (1 + rng.below(40), 1 + rng.below(110));
                let _ = write!(out, "\x1b[{row};{column}H");
            }
        }
        out.push_str(rng.pick(&WORDS));
        if rng.below(8) == 0 {
            out.push_str("\x1b[K");
        }
    }
    out.into_bytes()
}
