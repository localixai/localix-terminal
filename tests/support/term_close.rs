//! A terminal closed leaves nothing in its window: once the window lets its view go (past the
//! few ended terminals whose last screen it keeps), the view is gone from the window's
//! hierarchy and freed with its row pictures, even with a selection dragged past its edge, whose
//! display link would call it on.

use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Pointing, TermAnswer, TermProbe, TermViews};

use super::terminal_view::{opened, seen_in};

/// The closed terminal's id, and the ones ended after it, apart from the other scenes'.
const ID: u32 = 950;

/// More ended terminals than the window keeps the last screen of (`ENDED_KEPT`, 8).
const AFTER: u32 = 9;

fn views(browser: &Browser) -> TermViews {
    match browser.term_probe(0, TermProbe::Views) {
        Some(TermAnswer::Views(views)) => views,
        other => panic!("the window counted no terminal views: {other:?}"),
    }
}

pub(crate) fn scenario(browser: &Browser) {
    let session = opened(browser, ID, "seq 1 400; exec cat");
    let first = seen_in(browser, ID, |seen| seen.shown && seen.bytes > 0);
    let opened_views = views(browser);
    assert_eq!(opened_views.placed, opened_views.held, "{opened_views:?}");

    // A selection dragged past the bottom edge: the view scrolls on by a display link.
    let pointer = |pointing, row| TermProbe::Pointer {
        pointing,
        column: 2,
        row,
        flags: 0,
    };
    let below = first.lines + 3;
    for (pointing, row) in [(Pointing::Down, 2), (Pointing::Dragged, below)] {
        assert_eq!(
            browser.term_probe(ID, pointer(pointing, row)),
            Some(TermAnswer::Done)
        );
    }

    // Closed, then more terminals ended after it than the window keeps: its view is let go of.
    session.close();
    for n in 1..=AFTER {
        opened(browser, ID + n, "true").close();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while browser.term_probe(ID, TermProbe::Seen).is_some() {
        assert!(
            Instant::now() < deadline,
            "the window still holds the closed view"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let after = views(browser);
    assert_eq!(
        after.placed, after.held,
        "a closed terminal's view stays in its window: {after:?}"
    );
    assert_eq!(
        after.alive, after.held,
        "a closed terminal's view is never freed: {after:?}"
    );
}
