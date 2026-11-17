//! A tab carried over a terminal to split it, as a person carries one from the side: taken
//! with the mouse (the button's events through the app's queue), moved along a straight line
//! across the terminal's pane in small steps and let go over its right edge. Where it would
//! land, the tab itself stands: its own view, in sight, drawing its own text, in the half of
//! the pane the landing gives it, and the pane beside it makes room, the view itself in the
//! other half. The landing changes only as the pointer crosses from one edge's zone into
//! another's, never back and forth, and the views are drawn toward where they stand without
//! turning back. No terminal is told a grid while the tab is carried: the one shown keeps its
//! grid and text the whole way, and each is told one new grid as the tab lands. A terminal
//! carried over a page, and a page over a terminal, the same.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::{
    Browser, CarrySeen, PaneSeen, Panel, Pointing, TermAnswer, TermProbe, TermSeen,
};
use llxterm::surface::Surface;
use llxterm::terminal::Session;

use super::terminal_view::seen_in;

/// The project and its terminals' ids: apart from the other scenes' and the desk's.
const PROJECT: &str = "term-drop";
const SHOWN: u32 = 916;
const CARRIED: u32 = 917;
const PAGE: u32 = 918;
const OVER_PAGE: u32 = 919;
const UNDER: u32 = 920;
const CARRIED_PAGE: u32 = 921;

pub(crate) fn opened_in(browser: &Browser, id: u32, project: &str) -> (Arc<Session>, Panel) {
    let command = format!("printf 'pane {id}\\n'; exec sleep 60");
    running(browser, id, project, &command)
}

/// Terminal `id` of `project`, running `command`.
pub(crate) fn running(
    browser: &Browser,
    id: u32,
    project: &str,
    command: &str,
) -> (Arc<Session>, Panel) {
    let key = format!("t{id}");
    browser.group(&key, project);
    let session = Arc::new(Session::start(command, &std::env::temp_dir(), None, None).unwrap());
    let surface: Arc<dyn Surface> = Arc::clone(&session) as _;
    let panel = browser.term(id, command, surface);
    let wake = panel.waker();
    session.on_changed(move || {
        let _ = wake();
    });
    (session, panel)
}

/// The left button pressed, dragged or let go at a point of the window terminal `id` is in.
pub(crate) fn press(browser: &Browser, id: u32, pointing: Pointing, (x, y): (i32, i32)) {
    let answer = browser.term_probe(id, TermProbe::Press(pointing, x, y));
    assert_eq!(answer, Some(TermAnswer::Done));
    std::thread::sleep(Duration::from_millis(40));
}

/// The panes of the window terminal `id` is in, as a carried tab's landing moves them.
pub(crate) fn carried(browser: &Browser, id: u32) -> CarrySeen {
    match browser.term_probe(id, TermProbe::Carry) {
        Some(TermAnswer::Carry(seen)) => seen,
        other => panic!("the window's panes were not read: {other:?}"),
    }
}

/// Until no tab stands over the panes of terminal `id`'s window and every pane is drawn where
/// it stands, nothing of a layout still playing.
pub(crate) fn at_rest(browser: &Browser, id: u32) -> CarrySeen {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let seen = carried(browser, id);
        let still = |pane: &PaneSeen| pane.playing == 0 && pane.now == pane.rect;
        if seen.standing.is_none() && seen.panes.iter().all(still) {
            return seen;
        }
        assert!(
            Instant::now() < deadline,
            "the panes never rested: {seen:#?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What a terminal's program would see change: its grid, and how many grids it was told.
fn told(seen: &TermSeen) -> ((u16, u16), u64) {
    ((seen.columns, seen.lines), seen.resized)
}

/// How many times the landing went back to a place it had left while the pointer went on.
fn flips(places: &[[i32; 4]]) -> usize {
    let mut kept: Vec<[i32; 4]> = vec![];
    let mut back = 0;
    for place in places {
        if kept.last() == Some(place) {
            continue;
        }
        if kept.contains(place) {
            back += 1;
        }
        kept.push(*place);
    }
    back
}

/// The middle of tab `key`'s row in the side, in the window's points (the side's page lies under
/// the strip, from the pages' top, `top`).
pub(crate) fn row_of(browser: &Browser, key: &str, top: i32) -> (i32, i32) {
    let read = format!(
        "(r => [r.x + r.width / 2, r.y + r.height / 2])\
         (document.querySelector('[data-key=\"{key}\"]').getBoundingClientRect())"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let row: Vec<f64> = loop {
        if let Ok(row) = serde_json::from_value(browser.peek_tabs(&read).unwrap()) {
            break row;
        }
        assert!(Instant::now() < deadline, "{key} is not in the side");
        std::thread::sleep(Duration::from_millis(50));
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a point of the window, whole"
    )]
    let row = (row[0].round() as i32, row[1].round() as i32 + top);
    row
}

/// Tab `key` taken from its row and carried 10 steps to `to`, as a person takes it.
pub(crate) fn lift_to(browser: &Browser, id: u32, key: &str, top: i32, to: (i32, i32)) {
    let row = row_of(browser, key, top);
    press(browser, id, Pointing::Down, row);
    std::thread::sleep(Duration::from_millis(200));
    for step in 1..=10 {
        let at = (
            row.0 + step * (to.0 - row.0) / 10,
            row.1 + step * (to.1 - row.1) / 10,
        );
        press(browser, id, Pointing::Dragged, at);
    }
}

/// Pane `key` among `seen`'s.
fn pane<'a>(seen: &'a CarrySeen, key: &str) -> &'a PaneSeen {
    (seen.panes.iter())
        .find(|pane| pane.key == key)
        .unwrap_or_else(|| panic!("no pane {key}: {seen:#?}"))
}

/// Whether two rectangles (across, down, wide, tall) lie one over the other by more than a
/// point either way.
fn overlap(one: [i32; 4], other: [i32; 4]) -> bool {
    let over = |at: usize| {
        let ends = (one[at] + one[at + 2]).min(other[at] + other[at + 2]);
        ends - one[at].max(other[at]) > 1
    };
    over(0) && over(1)
}

/// The pointer moved to `at` and held still there: the panes of terminal `id`'s window
/// sampled about every frame, from the layout that follows the move until they are long
/// drawn where they stand.
pub(crate) fn sampled(browser: &Browser, id: u32, at: (i32, i32)) -> Vec<CarrySeen> {
    let places = |seen: &CarrySeen| -> Vec<(String, [i32; 4])> {
        (seen.panes.iter())
            .map(|pane| (pane.key.clone(), pane.rect))
            .collect()
    };
    let was = places(&carried(browser, id));
    press(browser, id, Pointing::Dragged, at);
    // The side's page tells of the pointer a frame after it moved, later still in a window
    // other windows cover: the samples are taken from the layout that follows it.
    let mut samples: Vec<CarrySeen> = vec![];
    let told = Instant::now() + Duration::from_secs(3);
    let mut deadline = None;
    while deadline.is_none_or(|deadline| Instant::now() < deadline) {
        let seen = carried(browser, id);
        if deadline.is_none() && places(&seen) != was {
            deadline = Some(Instant::now() + Duration::from_millis(600));
        }
        assert!(
            deadline.is_some() || Instant::now() < told,
            "the panes never moved for the pointer at {at:?}: {seen:#?}"
        );
        samples.push(seen);
        std::thread::sleep(Duration::from_millis(12));
    }
    let Some(last) = samples.last() else {
        return samples;
    };
    // From the sample the window was laid out for this landing in: the side's page tells of
    // the pointer a frame after it moved.
    let stands = |seen: &CarrySeen| {
        (last.panes.iter())
            .all(|pane| (seen.panes.iter()).any(|it| it.key == pane.key && it.rect == pane.rect))
    };
    let from = samples.iter().position(stands).unwrap_or(0);
    samples.split_off(from)
}

/// The pointer held still at `at` until the panes are drawn where they stand: sampled about
/// every frame, no pane's edge turns back on its way, none is drawn past the panes' part
/// (`area`) or over another, what each holds stays at its top left, and each ends where its
/// view stands, in full. How many of the samples found a pane still on its way.
fn settles(browser: &Browser, id: u32, at: (i32, i32), area: [i32; 4]) -> usize {
    let samples = sampled(browser, id, at);
    let Some(last) = samples.last() else {
        return 0;
    };
    let [x, top, wide, tall] = area;
    for seen in &samples {
        for (at, pane) in seen.panes.iter().enumerate() {
            let [left, up, across, down] = pane.now;
            assert!(
                left >= x - 1
                    && up >= top - 1
                    && left + across <= x + wide + 1
                    && up + down <= top + tall + 1,
                "{} drawn past the panes' part {area:?}: {seen:#?}",
                pane.key
            );
            // What it holds keeps its place from its top left, cut at its edge, never moved in it.
            assert!(
                (pane.holds[0] - left).abs() <= 1 && (pane.holds[1] - up).abs() <= 1,
                "what {} holds is drawn apart from it: {seen:#?}",
                pane.key
            );
            let others = seen.panes.get(at + 1..).unwrap_or_default();
            assert!(
                !others.iter().any(|other| overlap(pane.now, other.now)),
                "{} drawn over another pane: {seen:#?}",
                pane.key
            );
        }
    }
    for pane in &last.panes {
        let path: Vec<[i32; 4]> = (samples.iter())
            .filter_map(|seen| seen.panes.iter().find(|it| it.key == pane.key))
            .map(|it| it.now)
            .collect();
        for part in 0..4 {
            let steps: Vec<i32> = path
                .windows(2)
                .map(|two| two[1][part] - two[0][part])
                .collect();
            let up = steps.iter().any(|step| *step > 1);
            let down = steps.iter().any(|step| *step < -1);
            assert!(
                !(up && down),
                "{} turned back on its way: {path:?}",
                pane.key
            );
        }
        assert_eq!(pane.now, pane.rect, "{} not where it stands", pane.key);
        assert_eq!(
            (pane.playing, pane.opacity),
            (0, 100),
            "{} still moving: {last:#?}",
            pane.key
        );
    }
    let moving = |seen: &&CarrySeen| seen.panes.iter().any(|pane| pane.now != pane.rect);
    samples.iter().filter(moving).count()
}

/// Tab `key` taken from the side and carried along the middle of pane `shown` (which lies
/// over `area`), from its left to its right 8 points a step, then held over its right edge:
/// the tab's own view stands where it would land the whole way, beside the pane shown and
/// never over it, and that place moves only from one edge's zone into another's (its left,
/// then its right); what `told` reads never changes meanwhile. The panes as they stand before
/// the tab is let go.
fn carry(
    browser: &Browser,
    id: u32,
    (key, shown): (&str, &str),
    area: [i32; 4],
    told: &dyn Fn() -> String,
) -> CarrySeen {
    let [x, top, wide, tall] = area;
    let before = told();
    let laid = carried(browser, id).laid;
    let middle = top + tall / 2;
    let side_before = browser.peek_tabs("document.visibilityState").ok();
    lift_to(browser, id, key, top, (x + 20, middle));
    let mut places = vec![];
    let mut seen = CarrySeen::default();
    for across in (x + 20..x + wide - 20).step_by(8) {
        press(browser, id, Pointing::Dragged, (across, middle));
        assert_eq!(told(), before, "a program was told of {key} carried");
        seen = carried(browser, id);
        if seen.standing.as_deref() != Some(key) {
            continue;
        }
        let (lands, beside) = (pane(&seen, key), pane(&seen, shown));
        assert!(lands.seen && beside.seen, "a pane out of sight: {seen:#?}");
        places.push(lands.rect);
        // Side by side over the pane's whole part, a point between them, neither over the other.
        let (left, right) = match lands.rect[0] < beside.rect[0] {
            true => (lands.rect, beside.rect),
            false => (beside.rect, lands.rect),
        };
        assert_eq!(
            (left[0], left[0] + left[2] + 1, right[0] + right[2]),
            (x, right[0], x + wide),
            "the panes do not share the part: {seen:#?}"
        );
        assert_eq!(
            (left[1], left[3], right[1], right[3]),
            (top, tall, top, tall)
        );
    }
    // Whether the side's page is seen by the system: a window hidden behind others has its
    // pages' frames held back, and so the side's word of where the tab is.
    let seen_side = browser.peek_tabs("document.visibilityState").ok();
    let mut kept = places.clone();
    kept.dedup();
    eprintln!(
        "{key} carried across: {} flips, landings {kept:?}",
        flips(&places)
    );
    assert_eq!(
        flips(&places),
        0,
        "the landing went back and forth: {places:?}"
    );
    let right = *kept.last().unwrap_or_else(|| {
        panic!("nothing landed (side {side_before:?}, then {seen_side:?}): {seen:#?}")
    });
    assert!(
        right[0] > x + wide / 3 && right[0] + right[2] >= x + wide - 2,
        "the right edge never taken (side {seen_side:?}): {kept:?}"
    );
    assert!(
        kept.iter().any(|at| at[0] == x),
        "its left never taken: {kept:?}"
    );
    // From the right to the left, the top, the bottom and back to the right, in one step
    // each: every pane goes its way, and the tab stands over or under the pane shown too.
    let centre = x + wide / 2;
    let leftward = settles(browser, id, (x + 20, middle), area);
    let upward = settles(browser, id, (centre, top + 20), area);
    let above = carried(browser, id);
    assert_eq!(
        (pane(&above, key).rect, pane(&above, shown).rect[1]),
        ([x, top, wide, (tall - 1) / 2], top + (tall - 1) / 2 + 1),
        "not over the pane shown: {above:#?}"
    );
    let downward = settles(browser, id, (centre, top + tall - 20), area);
    let under = carried(browser, id);
    assert_eq!(
        (pane(&under, shown).rect[1], pane(&under, key).rect[1]),
        (top, top + tall - pane(&under, key).rect[3]),
        "not under the pane shown: {under:#?}"
    );
    let rightward = settles(browser, id, (x + wide - 20, middle), area);
    let last = carried(browser, id);
    assert_eq!(told(), before, "a program was told of {key} carried");
    // The window was laid out once for each landing, never for a step of the pointer.
    assert_eq!(
        last.laid - laid,
        u32::try_from(kept.len()).unwrap() + 4,
        "laid out more than once a landing: {kept:?}"
    );
    eprintln!(
        "{key}: on their way in {leftward}, {upward}, {downward} and {rightward} samples; {} \
         aims, the longest {} us, {} us each",
        last.aims,
        last.aim_most,
        last.aim_all / u64::from(last.aims.max(1))
    );
    last
}

/// A page's size as it lays itself out.
fn page_wide(browser: &Browser, panel: &Panel) -> i64 {
    browser
        .peek(panel, "innerWidth")
        .ok()
        .and_then(|wide| wide.as_i64())
        .unwrap_or(0)
}

fn page(browser: &Browser, id: u32, project: &str) -> Panel {
    browser.group(&format!("t{id}"), project);
    let html = format!("<!doctype html><title>page {id}</title><body>page {id}</body>");
    browser.panel(id, &format!("page {id}"), html)
}

/// Until `panel`'s page has laid itself out `wide` points wide.
fn page_laid(browser: &Browser, panel: &Panel, wide: i32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while page_wide(browser, panel) != i64::from(wide) {
        assert!(
            Instant::now() < deadline,
            "the page is {} wide, not {wide}",
            page_wide(browser, panel)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A terminal carried over a terminal: its own view stands where it would land, drawing its
/// own text; neither is told a grid until it lands, and then each is told one.
fn over_a_terminal(browser: &Browser) -> [i32; 4] {
    let (shown, shown_panel) = opened_in(browser, SHOWN, PROJECT);
    let (lands, lands_panel) = opened_in(browser, CARRIED, PROJECT);
    browser.go(PROJECT);
    browser.act(&format!("t{CARRIED}"), false).unwrap();
    let text = |id: u32| {
        move |seen: &TermSeen| {
            let first = seen.rows.first().map(|row| row.trim().to_string());
            seen.shown && first == Some(format!("pane {id}"))
        }
    };
    let carried_before = seen_in(browser, CARRIED, text(CARRIED));
    browser.act(&format!("t{SHOWN}"), false).unwrap();
    let before = seen_in(browser, SHOWN, text(SHOWN));
    let area = before.rect;
    let read = || {
        let shown = seen_in(browser, SHOWN, |_| true);
        let lands = seen_in(browser, CARRIED, |_| true);
        let first = shown.rows.first();
        format!("{:?} {first:?} {:?}", told(&shown), told(&lands))
    };
    let keys = (format!("t{CARRIED}"), format!("t{SHOWN}"));
    let last = carry(browser, SHOWN, (&keys.0, &keys.1), area, &read);
    // The tab standing where it would land is its own view: in sight, where the landing puts
    // it, its rows drawn, the grid it had.
    let standing = seen_in(browser, CARRIED, |seen| seen.shown);
    assert_eq!(standing.rect, pane(&last, &keys.0).rect);
    assert!(
        standing.layers > 0 && text(CARRIED)(&standing),
        "{standing:#?}"
    );
    assert_eq!(
        (standing.columns, standing.lines),
        (carried_before.columns, carried_before.lines)
    );
    let [x, top, wide, tall] = area;
    press(
        browser,
        SHOWN,
        Pointing::Up,
        (x + wide - 20, top + tall / 2),
    );
    let landed = seen_in(browser, SHOWN, |seen| seen.resized > before.resized);
    assert_eq!(landed.resized - before.resized, 1, "{landed:#?}");
    assert_eq!(landed.rect, pane(&last, &keys.1).rect, "moved as it landed");
    let beside = seen_in(browser, CARRIED, |seen| {
        seen.resized > carried_before.resized
    });
    assert_eq!(beside.resized - carried_before.resized, 1, "{beside:#?}");
    assert_eq!(beside.rect, pane(&last, &keys.0).rect, "moved as it landed");
    assert!(beside.shown && beside.columns < carried_before.columns);
    let rested = at_rest(browser, SHOWN);
    assert_eq!(rested.laid, last.laid, "laid out again for a tab let go");
    let (shown_now, beside_now) = (
        seen_in(browser, SHOWN, |_| true),
        seen_in(browser, CARRIED, |_| true),
    );
    assert_eq!(
        (shown_now.resized, beside_now.resized),
        (landed.resized, beside.resized),
        "a grid told again after the landing"
    );
    shown.close();
    lands.close();
    drop((shown_panel, lands_panel));
    area
}

pub(crate) fn scenario(browser: &Browser) {
    let area = over_a_terminal(browser);

    // A terminal carried over a page: the page is laid out in the half the landing leaves it.
    let paged = page(browser, PAGE, "term-drop-page");
    let (over_page, over_page_panel) = opened_in(browser, OVER_PAGE, "term-drop-page");
    browser.go("term-drop-page");
    browser.act(&format!("t{PAGE}"), false).unwrap();
    page_laid(browser, &paged, area[2]);
    // The terminal was never in sight: it has drawn no frame yet, so only the grids told count.
    let lands = seen_in(browser, OVER_PAGE, |_| true);
    let read = || format!("{}", seen_in(browser, OVER_PAGE, |_| true).resized);
    let keys = (format!("t{OVER_PAGE}"), format!("t{PAGE}"));
    let last = carry(browser, OVER_PAGE, (&keys.0, &keys.1), area, &read);
    page_laid(browser, &paged, pane(&last, &keys.1).rect[2]);
    let [x, top, wide, tall] = area;
    press(
        browser,
        OVER_PAGE,
        Pointing::Up,
        (x + wide - 20, top + tall / 2),
    );
    let landed = seen_in(browser, OVER_PAGE, |seen| seen.resized > lands.resized);
    assert_eq!(landed.resized - lands.resized, 1, "{landed:#?}");
    at_rest(browser, OVER_PAGE);
    over_page.close();
    drop((paged, over_page_panel));

    // A page carried over a terminal: the terminal keeps its grid until the page lands.
    let (under, under_panel) = opened_in(browser, UNDER, "term-drop-onto");
    let carried_page = page(browser, CARRIED_PAGE, "term-drop-onto");
    browser.go("term-drop-onto");
    browser.act(&format!("t{UNDER}"), false).unwrap();
    let alone = seen_in(browser, UNDER, |seen| seen.shown && seen.columns > 0);
    let read = || format!("{:?}", told(&seen_in(browser, UNDER, |_| true)));
    let keys = (format!("t{CARRIED_PAGE}"), format!("t{UNDER}"));
    let last = carry(browser, UNDER, (&keys.0, &keys.1), alone.rect, &read);
    page_laid(browser, &carried_page, pane(&last, &keys.0).rect[2]);
    let [x, top, wide, tall] = alone.rect;
    press(
        browser,
        UNDER,
        Pointing::Up,
        (x + wide - 20, top + tall / 2),
    );
    let landed = seen_in(browser, UNDER, |seen| seen.resized > alone.resized);
    assert_eq!(landed.resized - alone.resized, 1, "{landed:#?}");
    assert_eq!(landed.rect, pane(&last, &keys.1).rect);
    at_rest(browser, UNDER);
    under.close();
    drop((under_panel, carried_page));
}
