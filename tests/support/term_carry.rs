//! A tab carried and put back, carried out of its window, and carried along its list, as a
//! person does each with the mouse (the button's and Escape's events through the app's queue).
//! Escape over the panes puts every pane back where the layout keeps it and the tab out of
//! sight again, and no terminal is told a grid. Past the window's edge the tab comes out into a
//! window of its own, its view moved there as it is, and let go there it stays a window. Along
//! the list, the tab is moved to where it is let go, and the panes never move.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Pointing, TermAnswer, TermProbe};

use super::term_drop::{at_rest, carried, lift_to, opened_in, press, row_of};
use super::terminal_view::seen_in;

const PROJECT: &str = "term-carry";
const SHOWN: u32 = 922;
const CARRIED: u32 = 923;
const OUT: u32 = 924;
const THIRD: u32 = 925;

/// What the window tells whoever listens to it, as `key: what`.
fn listening(browser: &Browser) -> Arc<Mutex<Vec<String>>> {
    let heard = Arc::new(Mutex::new(vec![]));
    let told = Arc::clone(&heard);
    browser.listen(move |key, what| told.lock().unwrap().push(format!("{key}: {what:?}")));
    heard
}

/// Until the window has told `what` of `key`.
fn heard(heard: &Mutex<Vec<String>>, key: &str, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(heard.lock().unwrap().iter()).any(|it| it.starts_with(&format!("{key}: {what}"))) {
        assert!(
            Instant::now() < deadline,
            "never told {key} {what}: {heard:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    let (shown, shown_panel) = opened_in(browser, SHOWN, PROJECT);
    let (lands_session, carried_panel) = opened_in(browser, CARRIED, PROJECT);
    browser.go(PROJECT);
    browser.act(&format!("t{SHOWN}"), false).unwrap();
    let before = seen_in(browser, SHOWN, |seen| seen.shown && seen.columns > 0);
    let [x, top, wide, tall] = before.rect;
    let middle = top + tall / 2;

    // Escape with the tab over the left edge of the pane: the tab stood there, its own view in
    // sight; then the pane is where it was, the tab out of sight, and nothing was told a grid.
    let lands = seen_in(browser, CARRIED, |_| true);
    lift_to(
        browser,
        SHOWN,
        &format!("t{CARRIED}"),
        top,
        (x + 30, middle),
    );
    press(browser, SHOWN, Pointing::Dragged, (x + 40, middle));
    let open = carried(browser, SHOWN);
    assert_eq!(open.standing, Some(format!("t{CARRIED}")), "{open:#?}");
    let standing = seen_in(browser, CARRIED, |seen| seen.shown);
    assert_eq!(standing.rect, [x, top, (wide - 1) / 2, tall], "{open:#?}");
    let escape = TermProbe::KeyIn(53, "\u{1b}".into());
    assert_eq!(browser.term_probe(SHOWN, escape), Some(TermAnswer::Done));
    let gone = at_rest(browser, SHOWN);
    press(browser, SHOWN, Pointing::Up, (x + 40, middle));
    let after = seen_in(browser, SHOWN, |_| true);
    assert_eq!(
        (after.rect, after.columns, after.lines),
        (before.rect, before.columns, before.lines)
    );
    let put_back = seen_in(browser, CARRIED, |seen| !seen.shown);
    assert_eq!(
        (after.resized, put_back.resized),
        (before.resized, lands.resized),
        "a grid told for a tab put back"
    );
    assert_eq!(
        gone.laid - open.laid,
        1,
        "laid out more than once to go back"
    );

    // Along the list: the tab let go below the next one is moved there.
    let (third, third_panel) = opened_in(browser, THIRD, PROJECT);
    let told = listening(browser);
    // A window covered by others gets no pointer events in its pages: the run needs it seen.
    let side = browser.peek_tabs("document.visibilityState").ok();
    eprintln!("the side before the reorder: {side:?}");
    let from = row_of(browser, &format!("t{SHOWN}"), top);
    let below = row_of(browser, &format!("t{THIRD}"), top);
    press(browser, SHOWN, Pointing::Down, from);
    std::thread::sleep(Duration::from_millis(200));
    for step in 1..=12 {
        let y = from.1 + step * (below.1 + 12 - from.1) / 12;
        press(browser, SHOWN, Pointing::Dragged, (from.0, y));
    }
    press(browser, SHOWN, Pointing::Up, (from.0, below.1 + 12));
    heard(&told, &format!("t{SHOWN}"), "Moved");
    assert_eq!(
        carried(browser, SHOWN).laid,
        gone.laid,
        "the panes laid out for a tab along its list"
    );

    // Past the window's right edge: a window of its own with the tab's view in it, in sight;
    // let go, it stays. The source's pane is where it was, told no grid.
    let (out, out_panel) = opened_in(browser, OUT, PROJECT);
    browser.act(&format!("t{SHOWN}"), false).unwrap();
    seen_in(browser, SHOWN, |seen| seen.shown);
    let edge = x + wide;
    lift_to(browser, SHOWN, &format!("t{OUT}"), top, (edge - 40, middle));
    for step in 1..=8 {
        press(
            browser,
            SHOWN,
            Pointing::Dragged,
            (edge - 40 + step * 15, middle),
        );
    }
    let outside = seen_in(browser, OUT, |seen| seen.shown && seen.layers > 0);
    assert!(outside.rect[2] > 0, "{outside:#?}");
    press(browser, SHOWN, Pointing::Up, (edge + 80, middle));
    heard(&told, &format!("t{OUT}"), "Detached");
    let source = at_rest(browser, SHOWN);
    assert_eq!(source.standing, None);
    let source = seen_in(browser, SHOWN, |_| true);
    assert_eq!(
        (source.resized, source.rect),
        (before.resized, before.rect),
        "the source's pane moved or was told a grid"
    );
    assert_eq!(
        browser.term_probe(OUT, TermProbe::Close),
        Some(TermAnswer::Done)
    );

    for session in [shown, lands_session, out, third] {
        session.close();
    }
    drop((shown_panel, carried_panel, out_panel, third_panel));
}
