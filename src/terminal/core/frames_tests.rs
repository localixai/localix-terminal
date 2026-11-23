//! Tests of `frames`: while a program holds its frame back, the terminal shows the frame before,
//! to agents and to a view alike, and the new frame shows whole as the program lets go, however
//! its bytes were cut into reads; a resize, a second, or too many bytes let go of it; a begin
//! the core does not recognise holds nothing; a query inside a frame is answered as it ends.

use super::super::{Core, Render};
use super::{HOLD_MOST, KEPT_MOST};
use crate::surface::{CellPixels, Frame};

const BEGIN: &str = "\x1b[?2026h";
const RELEASE: &str = "\x1b[?2026l";

fn core(rows: u16, columns: u16) -> Core {
    match Core::new(rows, columns) {
        Ok(core) => core,
        Err(why) => panic!("a core of {rows}×{columns}: {why}"),
    }
}

/// The first row a view draws of `core` now.
fn drawn(render: &mut Render, core: &Core) -> String {
    let mut frame = Frame::default();
    assert!(render.frame(core, &mut frame, true));
    frame
        .rows
        .first()
        .map(|row| row.text.clone())
        .unwrap_or_default()
}

/// `bytes` fed whole, and cut into reads at every place.
fn every_cut(bytes: &str) -> Vec<Vec<Vec<u8>>> {
    let bytes = bytes.as_bytes();
    let mut cuts = vec![vec![bytes.to_vec()]];
    for at in 1..bytes.len() {
        cuts.push(vec![bytes[..at].to_vec(), bytes[at..].to_vec()]);
    }
    cuts.push(bytes.iter().map(|byte| vec![*byte]).collect());
    cuts
}

#[test]
fn a_held_frame_shows_the_one_before_until_it_is_let_go_of_whole() {
    let mut core = core(5, 20);
    let mut render = Render::new().unwrap();
    core.feed(format!("ready{BEGIN}\x1b[H\x1b[2Jhalf a fr").as_bytes());
    assert!(core.synchronized());
    // Agents and the view read the frame before, its cursor where it was.
    assert_eq!((core.text(), core.cursor()), ("ready".to_string(), (1, 6)));
    assert_eq!(core.search_text(), "ready");
    assert_eq!(drawn(&mut render, &core), "ready");
    core.feed(b"ame");
    assert_eq!(core.text(), "ready", "a frame spanning reads is still held");
    core.feed(RELEASE.as_bytes());
    assert!(!core.synchronized() && core.hold_left().is_none());
    assert_eq!(core.text(), "half a frame");
    assert_eq!(drawn(&mut render, &core), "half a frame");
}

#[test]
fn a_frame_shows_the_same_however_its_reads_were_cut() {
    // The begin and the release split across reads among them: a read that ends inside one
    // keeps what it has of it for the next.
    let program = format!("ready{BEGIN}\x1b[H\x1b[2Jnew{RELEASE}");
    for reads in every_cut(&program) {
        let mut core = core(5, 20);
        let (last, before) = reads.split_last().unwrap();
        for read in before {
            core.feed(read);
            assert!(
                ["", "r", "re", "rea", "read", "ready"].contains(&core.text().as_str()),
                "{reads:?}: {:?}",
                core.text()
            );
        }
        core.feed(last);
        assert_eq!(core.text(), "new", "{reads:?}");
        assert!(!core.synchronized(), "{reads:?}");
    }
}

#[test]
fn of_the_frames_in_one_read_the_one_left_open_is_held() {
    let mut core = core(5, 20);
    let frame = |text: &str| format!("{BEGIN}\x1b[H\x1b[2J{text}{RELEASE}");
    core.feed(format!("{}{}{BEGIN}\x1b[H\x1b[2Jhalf", frame("one"), frame("two")).as_bytes());
    assert_eq!(core.text(), "two");
    assert!(core.synchronized());
    core.feed(format!("{RELEASE}{}", frame("three")).as_bytes());
    assert_eq!(core.text(), "three");
    assert!(
        !core.synchronized(),
        "a read that ends every frame holds none"
    );
}

#[test]
fn a_resize_lets_the_held_frame_go_before_the_grid_changes() {
    let mut core = core(5, 20);
    core.feed(format!("before{BEGIN}\x1b[H\x1b[2Jheld").as_bytes());
    core.resize(6, 20, CellPixels::default()).unwrap();
    assert!(!core.synchronized(), "a resize lets go");
    assert_eq!(core.text(), "held");
    // What the program goes on drawing shows as it comes, and its release changes nothing.
    core.feed(format!(" on{RELEASE}").as_bytes());
    assert_eq!(core.text(), "held on");
}

#[test]
fn a_frame_held_past_its_most_is_let_go_of() {
    let mut core = core(5, 20);
    core.feed(format!("before{BEGIN}\x1b[H\x1b[2Jheld").as_bytes());
    assert!(core.hold_left().is_some_and(|left| left <= HOLD_MOST));
    assert!(!core.let_go_if_long(), "not yet");
    assert_eq!(core.text(), "before");
    crate::backoff(HOLD_MOST);
    assert_eq!(core.hold_left(), Some(std::time::Duration::ZERO));
    assert!(core.let_go_if_long());
    assert!(!core.let_go_if_long(), "once");
    assert_eq!(core.text(), "held");
    assert!(core.hold_left().is_none() && !core.synchronized());
    // A frame fed to past its most is let go of by the feed itself.
    core.feed(format!("{BEGIN}\x1b[H\x1b[2Jnext").as_bytes());
    crate::backoff(HOLD_MOST);
    core.feed(b" more");
    assert_eq!(core.text(), "next more");
    assert!(!core.synchronized());
}

#[test]
fn a_frame_of_too_many_bytes_is_let_go_of() {
    let mut core = core(5, 20);
    core.feed(format!("before{BEGIN}\x1b[H\x1b[2J").as_bytes());
    let line = "x".repeat(19) + "\r\n";
    core.feed(line.repeat(KEPT_MOST / line.len() + 1).as_bytes());
    assert!(!core.synchronized());
    assert!(core.text().starts_with("xxx"), "{:?}", core.text());
}

#[test]
fn a_begin_of_a_form_not_recognised_holds_nothing() {
    // Synchronized output asked among other modes: the frame is drawn as it comes, as a
    // terminal without synchronized output would, and nothing is ever held.
    let mut core = core(5, 20);
    core.feed(b"\x1b[?1049;2026hdrawn");
    assert!(core.alternate());
    assert!(!core.synchronized() && core.hold_left().is_none());
    assert_eq!(core.text(), "drawn");
    core.feed(format!(" on{RELEASE}").as_bytes());
    assert_eq!(core.text(), "drawn on");
}

#[test]
fn a_query_inside_a_held_frame_is_answered_as_the_frame_ends() {
    let mut core = core(5, 20);
    core.feed(format!("{BEGIN}abc\x1b[6n").as_bytes());
    assert_eq!(core.replies(), b"", "not while the frame is held");
    core.feed(RELEASE.as_bytes());
    assert_eq!(core.replies(), b"\x1b[1;4R");
}
