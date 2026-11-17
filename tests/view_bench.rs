//! The terminal canvas measured on macOS, without a window, against the bar its plan sets: a
//! 200 by 60 terminal at 2× in the default font (the system's monospace at 13 points) and in
//! Menlo; a full frame (every row laid out and drawn by Core Text) at most 6 ms, a frame of a
//! line of output (`seq`) and a frame of history scrolled by the wheel at least 55 a second, the
//! terminal's lock held at most 0.5 ms for the first half of an update; and what 1, 8 and 16
//! views' row images take. Run with `cargo test --release --test view_bench -- --ignored
//! --nocapture`; the figures go in the plan's item.

#![cfg(target_os = "macos")]
// Clippy 1.100 reads a test crate's helpers as test code, where clippy.toml lets a test unwrap,
// panic and print; clippy before it reads only a `#[test]` function so. An `expect` of these
// holds under one and fails under the other, so they are allowed.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "test code: a panic fails the test; a measurement prints its figures"
)]

#[path = "support/cast.rs"]
#[expect(
    dead_code,
    reason = "the bench plays a recording's output into a terminal of its own size"
)]
mod cast;

use std::path::Path;
use std::time::{Duration, Instant};

use cast::{Cast, Event};
use llxterm::browser::{TermLook, TermPainter};
use llxterm::surface::Frame;
use llxterm::terminal::{Core, Render};

const COLUMNS: u16 = 200;
const ROWS: u16 = 60;

/// A 200 by 60 terminal with a recording played into it, then a screen of coloured, styled text
/// in every cell (the worst a full frame can be), its history full.
fn busy(look: &TermLook) -> Core {
    let mut core = Core::new(ROWS, COLUMNS).unwrap();
    core.set_colors(&look.palette());
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal");
    for name in ["boxes", "wide", "top"] {
        for (_, event) in Cast::read(&fixtures.join(format!("{name}.cast"))).events {
            if let Event::Output(text) = event {
                core.feed(text.as_bytes());
            }
        }
    }
    core.feed(b"\x1b[?1049l\x1b[0m\r\n");
    for line in 0..10_000u32 {
        let colour = 31 + line % 7;
        let bold = if line % 3 == 0 { "1;" } else { "" };
        let words = format!(
            "\x1b[{bold}{colour}m{line:>6} fn draw(row: &Row) -> Paint {{ let x = 0x{line:04x}; }} \x1b[0m"
        );
        let mut row = words;
        while row.len() < 260 {
            row.push_str(" -> != == <= >= ");
        }
        core.feed(format!("{row}\r\n").as_bytes());
    }
    core
}

fn frame(render: &mut Render, core: &Core, full: bool) -> Frame {
    let mut frame = Frame::default();
    render.frame(core, &mut frame, full);
    frame
}

fn per(total: Duration, n: u32) -> f64 {
    total.as_secs_f64() * 1000.0 / f64::from(n)
}

fn measure(look: &TermLook) {
    let mut core = busy(look);
    let mut render = Render::new().unwrap();
    let mut painter = TermPainter::new(look).unwrap();
    // Full frames: every row laid out and drawn.
    let n = 40;
    let mut drawn = Duration::ZERO;
    let mut locked = Duration::ZERO;
    for _ in 0..n {
        let held = Instant::now();
        render.begin(&core).unwrap();
        locked += held.elapsed();
        let mut full = Frame::default();
        render.finish(&mut full, true);
        painter.forget();
        let started = Instant::now();
        let painted = painter.paint(&full);
        drawn += started.elapsed();
        assert!(painted.drawn > 0);
    }
    println!(
        "{}: full frame {:.2} ms; lock (begin) {:.3} ms",
        look.font,
        per(drawn, n),
        per(locked, n)
    );
    // A line of output a frame, as `seq` prints.
    let n = 300;
    let started = Instant::now();
    let mut redrawn = 0;
    for line in 0..n {
        core.feed(format!("{line}\r\n").as_bytes());
        redrawn += painter.paint(&frame(&mut render, &core, false)).drawn;
    }
    let seq = per(started.elapsed(), n);
    println!(
        "  seq: {seq:.2} ms a frame ({:.0} a second), {:.1} rows drawn a frame",
        1000.0 / seq,
        redrawn as f64 / f64::from(n)
    );
    // The wheel through history, a row a frame: every row dirty, all but one drawn before.
    let started = Instant::now();
    let mut redrawn = 0;
    for _ in 0..n {
        core.scroll(1);
        redrawn += painter.paint(&frame(&mut render, &core, false)).drawn;
    }
    let wheel = per(started.elapsed(), n);
    println!(
        "  wheel: {wheel:.2} ms a frame ({:.0} a second), {:.1} rows drawn a frame",
        1000.0 / wheel,
        redrawn as f64 / f64::from(n)
    );
    println!(
        "  one view's row images: {:.1} MB; its core: {:.1} MB",
        painter.bytes() as f64 / 1e6,
        core.memory() as f64 / 1e6
    );
    assert!(
        per(drawn, 40) <= 6.0 && seq <= 1000.0 / 55.0 && wheel <= 1000.0 / 55.0,
        "under the bar"
    );
}

#[test]
#[ignore = "a measure: run it on purpose, in release"]
fn the_canvas_is_fast_enough() {
    measure(&TermLook::new(true, 2.0));
    measure(&TermLook {
        font: "Menlo".into(),
        ..TermLook::new(true, 2.0)
    });
}

#[test]
#[ignore = "a measure: run it on purpose, in release"]
fn what_views_take() {
    let look = TermLook::new(true, 2.0);
    let core = busy(&look);
    let mut views = vec![];
    for count in [1usize, 8, 16] {
        while views.len() < count {
            let mut render = Render::new().unwrap();
            let mut painter = TermPainter::new(&look).unwrap();
            painter.paint(&frame(&mut render, &core, true));
            views.push((render, painter));
        }
        let images: usize = views.iter().map(|(_, painter)| painter.bytes()).sum();
        println!(
            "{count} views of 200x60 at 2x: row images {:.1} MB",
            images as f64 / 1e6
        );
    }
}
