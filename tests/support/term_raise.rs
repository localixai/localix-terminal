//! A tab carried under and over a terminal whose cursor is on its last row, as a person
//! carries one from the side. The pane it makes room in grows shorter, and its text is raised
//! as far as keeps the cursor's row in view, as while a divider is held; the text goes up with
//! the pane's edge, by the same spring, and comes down with it again: never at once, a
//! screen's half in one frame, while the edge is still on its way.

use llxterm::browser::{Browser, CarrySeen, PaneSeen, Pointing, TermAnswer, TermProbe};

use super::term_drop::{at_rest, carried, lift_to, opened_in, press, running, sampled};
use super::terminal_view::seen_in;

/// The project and its terminals' ids: apart from the other scenes' and the desk's.
const PROJECT: &str = "term-raise";
const FULL: u32 = 922;
const CARRIED: u32 = 923;

/// How far apart, in points, the pane's lower edge and the row the text is raised for may be
/// drawn on the way: the room under the last row of a grid, a row and its gutter at most.
const SLACK: i32 = 28;

/// Pane `key` among `seen`'s.
fn pane<'a>(seen: &'a CarrySeen, key: &str) -> &'a PaneSeen {
    (seen.panes.iter())
        .find(|pane| pane.key == key)
        .unwrap_or_else(|| panic!("no pane {key}: {seen:#?}"))
}

/// The pointer moved to `at`: pane `key`'s text ends raised by `raised` points (none: where it
/// lies), and on the way it is never raised at once nor let down at once: what it is raised
/// by follows the pane's lower edge, so the row it is raised for stays by that edge.
fn rides(browser: &Browser, key: &str, at: (i32, i32), raised: bool) -> i32 {
    let samples = sampled(browser, FULL, at);
    let last = samples.last().expect("the panes were sampled");
    let end = pane(last, key);
    assert_eq!(end.raised > 0, raised, "{key} at {at:?}: {last:#?}");
    assert_eq!((end.now, end.playing), (end.rect, 0), "{last:#?}");
    // The pane's lower edge from the top of what it holds, and how far that is raised: their
    // sum is where the row kept in view lies from that edge, the same all the way.
    let kept = |pane: &PaneSeen| pane.now[1] + pane.now[3] - pane.holds[1] + pane.raised;
    let path: Vec<(i32, i32)> = (samples.iter())
        .map(|seen| pane(seen, key))
        .map(|pane| (pane.raised, kept(pane)))
        .collect();
    for (_, at_edge) in &path {
        assert!(
            (at_edge - kept(end)).abs() <= SLACK,
            "{key}'s text left its pane's edge on the way (raised, edge): {path:?}"
        );
    }
    end.raised
}

pub(crate) fn scenario(browser: &Browser) {
    let command = "seq 1 200; exec sleep 60";
    let (full, full_panel) = running(browser, FULL, PROJECT, command);
    let (lands, lands_panel) = opened_in(browser, CARRIED, PROJECT);
    browser.go(PROJECT);
    browser.act(&format!("t{FULL}"), false).unwrap();
    let before = seen_in(browser, FULL, |seen| seen.shown && seen.history > 100);
    let [x, top, wide, tall] = before.rect;
    let (key, shown) = (format!("t{CARRIED}"), format!("t{FULL}"));
    let (centre, middle) = (x + wide / 2, top + tall / 2);

    // Beside it first, on the left it came in over: the pane keeps its height, and nothing
    // is raised.
    lift_to(browser, FULL, &key, top, (x + 30, middle));
    let beside = carried(browser, FULL);
    assert_eq!(
        beside.standing.as_deref(),
        Some(key.as_str()),
        "{beside:#?}"
    );
    assert_eq!(pane(&beside, &shown).raised, 0, "{beside:#?}");
    // Under it: the pane is half as tall, and its text goes up with its edge.
    let under = rides(browser, &shown, (centre, top + tall - 20), true);
    assert!(under > tall / 3, "raised {under} of {tall}");
    // Beside it again: the text comes down with the edge.
    rides(browser, &shown, (x + 20, middle), false);
    // Over it: the pane goes down and its text stays where it is on the screen.
    let over = rides(browser, &shown, (centre, top + 20), true);
    assert!(
        (over - under).abs() <= 2,
        "raised {over} over it, {under} under"
    );
    // Put back with Escape: the text comes down as the pane grows back, and no grid was told.
    let escape = TermProbe::KeyIn(53, "\u{1b}".into());
    assert_eq!(browser.term_probe(FULL, escape), Some(TermAnswer::Done));
    let rested = at_rest(browser, FULL);
    assert_eq!(pane(&rested, &shown).raised, 0, "{rested:#?}");
    press(browser, FULL, Pointing::Up, (centre, top + 20));
    let after = seen_in(browser, FULL, |_| true);
    assert_eq!(
        (after.resized, after.rect),
        (before.resized, before.rect),
        "a grid told for a tab put back"
    );
    full.close();
    lands.close();
    drop((full_panel, lands_panel));
}
