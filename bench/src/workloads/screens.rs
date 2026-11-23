//! Workloads of whole screens: a full-screen program's frames, drawn on the other screen inside
//! synchronized updates (mode 2026) as a program that redraws often does; and the app's own
//! recordings of real sessions (`tests/fixtures/terminal/*.cast`), played one after another
//! into the bench's terminal whatever size each was recorded at.

use std::fmt::Write;
use std::path::Path;

use super::{Rng, WORDS, line};
use crate::cast::{Cast, Event};
use crate::{COLUMNS, ROWS};

/// Frames of a list program on the other screen: a title row, rows of a table each in its own
/// colours with a highlighted one, and a status row in reverse video; each frame drawn whole
/// from the top, every row addressed and erased to its end, inside a synchronized update.
pub(super) fn tui(size: usize) -> Vec<u8> {
    let mut rng = Rng::new(0x7e1e);
    let mut out = String::from("\x1b[?1049h\x1b[?25l");
    let mut frame = 0u64;
    while out.len() < size {
        out.push_str("\x1b[?2026h\x1b[H");
        let _ = write!(
            out,
            "\x1b[1;37;44m frame {frame:>8} {:>100}\x1b[0m",
            "tasks"
        );
        let chosen = rng.below(usize::from(ROWS) - 2);
        for row in 0..usize::from(ROWS) - 2 {
            let _ = write!(out, "\x1b[{};1H", row + 2);
            if row == chosen {
                out.push_str("\x1b[7m");
            }
            let cpu = rng.below(1000);
            let colour = 31 + rng.below(7);
            let name = rng.pick(&WORDS);
            let _ = write!(
                out,
                "\x1b[{colour}m{:>6}\x1b[39m {name:<16} \x1b[38;5;{}m{:>3}.{}%\x1b[0m ",
                rng.below(99_999),
                rng.below(256),
                cpu / 10,
                cpu % 10,
            );
            let used = 34;
            let words = line(&mut rng, usize::from(COLUMNS) - used - 1);
            out.push_str(&words);
            out.push_str("\x1b[K\x1b[0m");
        }
        let _ = write!(
            out,
            "\x1b[{};1H\x1b[7m q quit  / find  ↑↓ move {:>90}\x1b[0m",
            ROWS, frame
        );
        out.push_str("\x1b[?2026l");
        frame += 1;
    }
    out.into_bytes()
}

/// The app's recordings in name order, their output one after another, again from the first
/// until `size` is reached; nothing where there are no recordings. A recording's resizes are
/// left out: every core keeps the bench's size.
pub(super) fn recorded(size: usize) -> Vec<u8> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/terminal");
    let mut paths: Vec<_> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cast"))
        .collect();
    paths.sort();
    let mut once = Vec::new();
    for path in &paths {
        for (_, event) in Cast::read(path).events {
            if let Event::Output(text) = event {
                once.extend_from_slice(text.as_bytes());
            }
        }
    }
    if once.is_empty() {
        return once;
    }
    let mut out = Vec::with_capacity(size + once.len());
    while out.len() < size {
        out.extend_from_slice(&once);
    }
    out
}
