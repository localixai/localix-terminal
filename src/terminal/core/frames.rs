//! A frame a program holds back while it draws it (synchronized output: `ESC[?2026h` begins it,
//! `ESC[?2026l` lets it go). Its bytes are kept here from the begin on and not written to the
//! terminal, so the terminal goes on holding the frame before, for a view and for agents alike,
//! with nothing read or copied for them; as the program lets go, the frame is written whole.
//!
//! Each read is joined to what is kept and cut the same way: everything up to the last release
//! is written, and from a begin after it on is kept; a read that ends inside a begin or a release
//! keeps what it has of it for the next. A frame is let go of unended after `HOLD_MOST`, as a
//! resize comes, or past `KEPT_MOST` bytes. A query inside a kept frame is answered as the frame
//! is written. The library is never left holding anything itself: a begin it took in a form not
//! recognised here (among other modes, `ESC[?1049;2026h`) is turned off at once, and that frame
//! is drawn as it comes, as by a terminal without synchronized output.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use memchr::memmem;

use ghostty_vt_sys as vt;

use super::Core;

/// The longest a program's hold on its frame is believed: one that never lets go is let go of
/// after this, as a terminal draws again after a while.
pub(super) const HOLD_MOST: Duration = Duration::from_secs(1);

/// The most bytes of a frame kept: a program that holds one this long is not drawing a frame.
pub(super) const KEPT_MOST: usize = 8 << 20;

/// What begins and what ends a frame; they differ in their last byte alone, so what comes
/// before it (`MARK`) is what is looked for.
const BEGIN: &[u8] = b"\x1b[?2026h";
const RELEASE: &[u8] = b"\x1b[?2026l";
const MARK: &[u8] = b"\x1b[?2026";

/// The bytes not yet written: a frame held from its begin on, or the start of a begin or a
/// release that a read ended in.
#[derive(Debug, Default)]
pub(super) struct Kept {
    bytes: Vec<u8>,
    /// When the frame kept began; none while what is kept opens no frame.
    since: Option<Instant>,
}

/// The marks in `bytes` that begin at `from` or after, in one pass: where the last release
/// ends, and where the first begin after it begins (after `from`, with no release).
fn marks(bytes: &[u8], from: usize) -> (Option<usize>, Option<usize>) {
    static FINDER: LazyLock<memmem::Finder<'static>> = LazyLock::new(|| memmem::Finder::new(MARK));
    let (mut released, mut begun) = (None, None);
    let rest = bytes.get(from..).unwrap_or_default();
    for at in FINDER.find_iter(rest).map(|found| from + found) {
        match bytes.get(at + MARK.len()) {
            Some(b'l') => (released, begun) = (Some(at + RELEASE.len()), None),
            Some(b'h') if begun.is_none() => begun = Some(at),
            _ => {}
        }
    }
    (released, begun)
}

/// How many bytes at the end of `bytes` begin a begin or a release without ending it.
fn started(bytes: &[u8]) -> usize {
    let common = BEGIN.len() - 1;
    (1..=common)
        .rev()
        .find(|&len| BEGIN.get(..len).is_some_and(|start| bytes.ends_with(start)))
        .unwrap_or(0)
}

/// How many of `bytes` are written now, and whether the rest is a frame held (else it is the
/// start of a mark, or nothing). `held` says `bytes` open with a frame already held, whose first
/// `seen` bytes held no release: only a release that ends after them can end it.
fn cut(bytes: &[u8], held: bool, seen: usize) -> (usize, bool) {
    let from = match held {
        true => seen.saturating_sub(RELEASE.len() - 1),
        false => 0,
    };
    let (released, begun) = marks(bytes, from);
    if held && released.is_none() {
        return (0, true);
    }
    match begun {
        Some(at) => (at, true),
        None => {
            let rest = bytes.get(released.unwrap_or(0)..).unwrap_or_default();
            (bytes.len() - started(rest), false)
        }
    }
}

impl Core {
    /// Takes in a program's output, as a terminal does: a frame it holds back is kept until it
    /// lets go (above).
    pub fn feed(&mut self, bytes: &[u8]) {
        let held = self.kept.since.is_some();
        let (written, holds) = if self.kept.bytes.is_empty() {
            let (written, holds) = cut(bytes, false, 0);
            let (now, rest) = bytes.split_at(written);
            self.write(now);
            self.kept.bytes.extend_from_slice(rest);
            (written, holds)
        } else {
            let mut all = std::mem::take(&mut self.kept.bytes);
            let seen = all.len();
            all.extend_from_slice(bytes);
            let (written, holds) = cut(&all, held, seen);
            self.write(all.get(..written).unwrap_or_default());
            all.drain(..written);
            self.kept.bytes = all;
            (written, holds)
        };
        // The frame held is the same one while nothing before it was written.
        self.kept.since = match (holds, held && written == 0) {
            (false, _) => None,
            (true, true) => self.kept.since,
            (true, false) => Some(Instant::now()),
        };
        if self.kept.bytes.len() > KEPT_MOST || self.hold_left() == Some(Duration::ZERO) {
            self.let_go();
        }
    }

    /// Writes `bytes` into the terminal. A frame begun in a form not recognised is let go of
    /// at once (above), so the library holds nothing of its own.
    fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // SAFETY: the bytes are read only during the call; the core takes any bytes at all.
        unsafe { vt::ghostty_terminal_vt_write(self.term, bytes.as_ptr(), bytes.len()) };
        self.found.stale = true;
        if self.mode(2026) {
            let off = vt::TerminalModeConfig {
                mode: 2026,
                value: false,
            };
            let _ = self.set(vt::TerminalOption::MODE, &off);
        }
    }

    /// The frame held is written as it is, as though the program had let go; whether one was.
    pub(super) fn let_go(&mut self) -> bool {
        if self.kept.since.take().is_none() {
            return false;
        }
        let bytes = std::mem::take(&mut self.kept.bytes);
        self.write(&bytes);
        true
    }

    /// Whether the program holds a frame back now (synchronized output): the screen shows the
    /// frame before it until it lets go.
    pub fn synchronized(&self) -> bool {
        self.kept.since.is_some()
    }

    /// How long until the frame a program holds is let go of (`HOLD_MOST` after it began);
    /// none while it holds nothing.
    pub fn hold_left(&self) -> Option<Duration> {
        (self.kept.since).map(|since| HOLD_MOST.saturating_sub(since.elapsed()))
    }

    /// Lets go of a frame held past `HOLD_MOST`, as the program would have; whether there was one.
    pub fn let_go_if_long(&mut self) -> bool {
        self.hold_left() == Some(Duration::ZERO) && self.let_go()
    }
}

#[cfg(test)]
#[path = "frames_tests.rs"]
mod tests;
