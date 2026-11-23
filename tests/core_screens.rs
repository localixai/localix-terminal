//! The terminal core on recordings of real sessions (`tests/fixtures/terminal/*.cast`, made with
//! `LLXTERM_RECORD`): each recording is fed to the core, and the core's last screen (its size,
//! cursor, which screen, how much history and the text in view) must be the one kept beside it
//! (`*.screen`). `LLXTERM_WRITE_SCREENS=1` writes them anew, to be read before they are kept.
//!
//! The core is fed the raw recording. In the app the session's filter decides what reaches the
//! core (no string sequence but a shell's marks does), so this holds the emulator to what it
//! drew, not what an agent reads.

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

use std::path::{Path, PathBuf};

use cast::{Cast, Event};
use llxterm::surface::CellPixels;
use llxterm::terminal::Core;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal")
}

fn recordings() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cast"))
        .collect();
    found.sort();
    found
}

/// The core after the whole of `cast`.
fn played(cast: &Cast) -> Core {
    let mut core = Core::new(cast.rows, cast.columns).unwrap();
    for (_, event) in &cast.events {
        match event {
            Event::Output(text) => core.feed(text.as_bytes()),
            Event::Resize(rows, columns) => {
                core.resize(*rows, *columns, CellPixels::default()).unwrap()
            }
        }
    }
    core
}

/// The core's screen as its `*.screen` file keeps it.
fn screen_file(core: &Core) -> String {
    let (row, column) = core.cursor();
    let (rows, columns) = core.size();
    let screen = if core.alternate() {
        "alternate"
    } else {
        "main"
    };
    format!(
        "size {rows}x{columns}\ncursor {row} {column}\nscreen {screen}\nhistory {}\n---\n{}\n",
        core.history(),
        core.text()
    )
}

/// The screens were written from the earlier core and are the same from libghostty-vt, every
/// one: none was written again. Each is also what the window's former page drew from the
/// recording.
#[test]
fn the_cores_last_screen_of_each_recording_is_as_kept() {
    let write = std::env::var_os("LLXTERM_WRITE_SCREENS").is_some_and(|v| v == "1");
    let mut broken = vec![];
    for path in recordings() {
        let screen = screen_file(&played(&Cast::read(&path)));
        let kept = path.with_extension("screen");
        if write {
            std::fs::write(&kept, &screen).unwrap();
            continue;
        }
        match std::fs::read_to_string(&kept) {
            Ok(was) if was == screen => {}
            Ok(was) => broken.push(format!("{}:\nkept {was}\nnow  {screen}", kept.display())),
            Err(why) => broken.push(format!("{}: {why}", kept.display())),
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n\n"));
}

#[test]
fn every_recording_is_small_and_has_its_screen() {
    let all = recordings();
    assert!(all.len() >= 10, "{} recordings", all.len());
    for path in all {
        let size = std::fs::metadata(&path).unwrap().len();
        assert!(size <= 200 * 1024, "{}: {size} bytes", path.display());
        assert!(path.with_extension("screen").exists(), "{}", path.display());
    }
}
