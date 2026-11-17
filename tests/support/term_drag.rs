//! A split of two terminals as a person moves it: the divider dragged 200 points and let go,
//! the window made narrower and wider again a step at a time (as its zoom or an agent sizes it),
//! and its edge held while it is made shorter and let go. Each program counts the size changes
//! its terminal sends it (SIGWINCH); the views count the grids they told their terminals and the
//! frames that left a row of theirs empty. While a divider or the window's edge is held, each
//! terminal keeps its grid, its text in place (raised only to keep the cursor's row in view),
//! nothing past its pane and nothing animated; let go, each is told its new grid once. A window
//! resized step by step tells each grid once a step, never a size between two. A pane put by
//! for another's zoom keeps its grid until it is back.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Pointing, TermAnswer, TermProbe, TermSeen};
use llxterm::layout::Tree;

use super::term_split::{arranged_in, opened_in};
use super::terminal_view::seen_in;

/// The project and its terminals' ids: apart from the other scenes' and the desk's.
const PROJECT: &str = "term-drag";
const LEFT: u32 = 914;
const RIGHT: u32 = 915;

/// A program that writes `first` and a line to `count` on each size change.
fn counting(first: &str, count: &Path) -> String {
    let count = count.display();
    format!(
        "trap 'echo w >> {count}' WINCH; {first}; \
         while :; do sleep 60 & wait $!; kill $! 2>/dev/null; done"
    )
}

/// How many size changes the program counted, once nothing more comes for a moment (its trap
/// runs as it is told).
fn winches(count: &Path) -> usize {
    let read = || std::fs::read_to_string(count).map_or(0, |text| text.lines().count());
    let mut last = (read(), Instant::now());
    let deadline = Instant::now() + Duration::from_secs(5);
    while last.1.elapsed() < Duration::from_millis(400) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(40));
        let now = read();
        if now != last.0 {
            last = (now, Instant::now());
        }
    }
    last.0
}

fn seen(browser: &Browser, id: u32) -> TermSeen {
    seen_in(browser, id, |_| true)
}

fn probe(browser: &Browser, probe: TermProbe) {
    assert_eq!(browser.term_probe(LEFT, probe), Some(TermAnswer::Done));
}

/// The window's content made `width` by `height` points: the size it had.
fn sized(browser: &Browser, width: u32, height: u32) -> (u32, u32) {
    match browser.term_probe(LEFT, TermProbe::Window(width, height)) {
        Some(TermAnswer::Window(was_wide, was_tall)) => (was_wide, was_tall),
        other => panic!("the window was not sized: {other:?}"),
    }
}

/// What a pane went through between two readings: the grids it told, the size changes its
/// program heard, and the frames that left a row empty.
#[derive(Debug)]
struct Went {
    resized: u64,
    winched: usize,
    blank: u64,
}

/// Each pane's reading now, with its program's count.
fn read(browser: &Browser, counts: &[PathBuf; 2]) -> [(TermSeen, usize); 2] {
    [(LEFT, 0), (RIGHT, 1)].map(|(id, at)| (seen(browser, id), winches(&counts[at])))
}

fn went(before: &[(TermSeen, usize); 2], after: &[(TermSeen, usize); 2]) -> [Went; 2] {
    [0, 1].map(|at| Went {
        resized: after[at].0.resized - before[at].0.resized,
        winched: after[at].1 - before[at].1,
        blank: after[at].0.blank - before[at].0.blank,
    })
}

/// Each pane as a step of a move leaves it: drawn inside its pane, nothing animated.
fn step(browser: &Browser) -> [TermSeen; 2] {
    let both = [seen(browser, LEFT), seen(browser, RIGHT)];
    for one in &both {
        assert!(one.clips, "a pane draws past its bounds: {one:#?}");
        assert_eq!(
            one.moving, 0,
            "a layer animated as the split moved: {one:#?}"
        );
    }
    both
}

/// The divider taken with the mouse as a person takes it (the button's events through the
/// app's queue, to whatever view lies under them) and dragged 100 points right: the panes move
/// while it is held, each grid kept, and each terminal is told its new grid once it is let go.
fn by_hand(browser: &Browser, counts: &[PathBuf; 2]) {
    let before = read(browser, counts);
    let ([x, top, wide, tall], [right, ..]) = (before[0].0.rect, before[1].0.rect);
    let (at, down) = ((x + wide + right) / 2, top + tall / 2);
    // A person's pace: a move every frame or two, the page given time to take the press.
    let press = |pointing: Pointing, across: i32| {
        probe(browser, TermProbe::Press(pointing, at + across, down));
        std::thread::sleep(Duration::from_millis(40));
    };
    press(Pointing::Down, 0);
    std::thread::sleep(Duration::from_millis(200));
    for step in 1..=20 {
        press(Pointing::Dragged, step * 5);
    }
    let wider = wide + 60;
    let held = seen_in(browser, LEFT, |seen| seen.rect[2] >= wider);
    assert_eq!(
        held.columns, before[0].0.columns,
        "a grid told while the divider is held"
    );
    press(Pointing::Up, 100);
    let columns = before[0].0.columns + 8;
    seen_in(browser, LEFT, |seen| seen.columns >= columns);
    let moved = went(&before, &read(browser, counts));
    eprintln!("divider dragged 100 points by hand: {moved:?}");
    assert!(once(&moved), "{moved:?}");
    // The divider's page took the window's keys with the press: given back, as the scenes after
    // this one find the window.
    probe(browser, TermProbe::Focus(true));
}

/// Once: told one grid, its program one size change, no row left empty.
fn once(went: &[Went; 2]) -> bool {
    (went.iter()).all(|pane| pane.resized == 1 && pane.winched == 1 && pane.blank == 0)
}

pub(crate) fn scenario(browser: &Browser) {
    let dir = std::env::temp_dir().join(format!("llxterm-drag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let counts = [dir.join("left"), dir.join("right")];
    let left_command = counting("printf 'pane 914\\nsecond line\\n'", &counts[0]);
    let (left, left_panel) = opened_in(browser, (PROJECT, LEFT), false, &left_command);
    browser.go(PROJECT);
    browser.act(&format!("t{LEFT}"), false).unwrap();
    let found = sized(browser, 1200, 800);
    seen_in(browser, LEFT, |seen| seen.shown && seen.columns > 0);
    // The right pane's cursor is on its last row, where a shorter pane would lose it.
    let right_command = counting("seq 1 200", &counts[1]);
    let (right, right_panel) = opened_in(browser, (PROJECT, RIGHT), true, &right_command);
    let mut tree = Tree::leaf(format!("t{LEFT}"));
    assert!(tree.split(&format!("t{LEFT}"), format!("t{RIGHT}"), true, true));
    let focused = format!("t{LEFT}");
    browser.arranged(vec![0], arranged_in(PROJECT, &tree, &focused, false));
    let text = |seen: &TermSeen| {
        seen.rows
            .first()
            .is_some_and(|row| row.trim() == "pane 914")
    };
    seen_in(browser, LEFT, |seen| seen.shown && text(seen));
    seen_in(browser, RIGHT, |seen| seen.shown && seen.history > 100);
    by_hand(browser, &counts);
    let before = read(browser, &counts);

    // The divider dragged 200 points to the right in steps of 4, as the strip says it moves:
    // each terminal keeps its grid and its text until it is let go of.
    for moved in (4..=200).step_by(4) {
        browser.click(&format!("split-move 0 {moved}"));
        let [one, two] = step(browser);
        assert!(
            text(&one),
            "the left pane's text moved while dragged: {:?}",
            one.rows
        );
        let grids = (one.columns, two.columns);
        assert_eq!(
            grids,
            (before[0].0.columns, before[1].0.columns),
            "a grid told mid-drag"
        );
    }
    browser.click("split-move 0 200 done");
    let wider = before[0].0.columns + 20;
    seen_in(browser, LEFT, |seen| seen.columns >= wider);
    let dragged = read(browser, &counts);
    let drag = went(&before, &dragged);
    eprintln!("divider dragged 200 points: {drag:?}");
    assert!(once(&drag), "{drag:?}");

    // The window made 200 points narrower and wider again, 10 a step, each step laid out:
    // each grid is told once a step at most, and never a size between two (it only narrows
    // as the window narrows, and only widens as it widens).
    let mut last = [dragged[0].0.clone(), dragged[1].0.clone()];
    let mut was = 1200;
    for width in (0..=20).chain((0..20).rev()).map(|at| 1200 - at * 10) {
        let narrower = width <= was;
        was = width;
        sized(browser, width, 800);
        let now = step(browser);
        for (before, after) in last.iter().zip(&now) {
            assert!(
                after.resized - before.resized <= 1,
                "{before:#?} {after:#?}"
            );
            let back = match narrower {
                true => after.columns > before.columns,
                false => after.columns < before.columns,
            };
            assert!(!back, "a grid between two sizes: {before:#?} {after:#?}");
        }
        last = now;
    }
    let resized = read(browser, &counts);
    let window = went(&dragged, &resized);
    eprintln!("window resized 40 steps: {window:?}");
    assert!(window.iter().all(|pane| pane.blank == 0), "{window:?}");

    // The window's edge held while it is made 200 points shorter: the grids are kept, the right
    // pane's rows raised so its cursor's row stays in it; let go, each is told once.
    probe(browser, TermProbe::Edge(true));
    for height in (600..800).rev().step_by(10) {
        sized(browser, 1200, height);
        let [one, two] = step(browser);
        assert_eq!(
            (one.lines, two.lines),
            (resized[0].0.lines, resized[1].0.lines)
        );
        assert!(
            text(&one),
            "the left pane's text moved while held: {:?}",
            one.rows
        );
    }
    let held = seen(browser, RIGHT);
    assert!(
        held.lifted > 100,
        "the right pane's cursor row is out of view: {held:#?}"
    );
    probe(browser, TermProbe::Edge(false));
    let shorter = resized[0].0.lines - 10;
    seen_in(browser, LEFT, |seen| seen.lines <= shorter);
    let let_go = seen_in(browser, RIGHT, |seen| seen.lines <= shorter);
    assert_eq!(let_go.lifted, 0, "{let_go:#?}");
    let edge = went(&resized, &read(browser, &counts));
    eprintln!("window's edge held, 200 points shorter: {edge:?}");
    assert!(once(&edge), "{edge:?}");

    // Zoomed on the left pane and back: the right one, out of sight meanwhile, keeps its grid
    // and its program hears nothing; the left is told the whole tab's grid and its own again.
    let unzoomed = read(browser, &counts);
    browser.click("key zoom");
    browser.arranged(vec![0], arranged_in(PROJECT, &tree, &focused, true));
    seen_in(browser, RIGHT, |seen| !seen.shown);
    seen_in(browser, LEFT, |seen| {
        seen.columns > unzoomed[0].0.columns + 20
    });
    browser.click("key zoom");
    browser.arranged(vec![0], arranged_in(PROJECT, &tree, &focused, false));
    seen_in(browser, RIGHT, |seen| seen.shown && seen.bytes > 0);
    seen_in(browser, LEFT, |seen| seen.columns == unzoomed[0].0.columns);
    let zoom = went(&unzoomed, &read(browser, &counts));
    eprintln!("zoomed and back: {zoom:?}");
    assert_eq!((zoom[0].resized, zoom[0].winched), (2, 2), "{zoom:?}");
    assert_eq!((zoom[1].resized, zoom[1].winched), (0, 0), "{zoom:?}");
    // AppKit's word that a live resize started, with no end after it: the window is in none,
    // so a divider dragged after it still tells each terminal its grid once let go.
    probe(browser, TermProbe::EdgeTold);
    let told = read(browser, &counts);
    for moved in (10..=100).step_by(10) {
        browser.click(&format!("split-move 0 -{moved}"));
    }
    browser.click("split-move 0 -100 done");
    let narrower = told[0].0.columns - 8;
    seen_in(browser, LEFT, |seen| seen.columns <= narrower);
    let stray = went(&told, &read(browser, &counts));
    eprintln!("divider dragged after a stray start of a live resize: {stray:?}");
    assert!(once(&stray), "{stray:?}");

    // The window as the scene found it, for the scenes after it.
    sized(browser, found.0, found.1);
    left.close();
    right.close();
    drop((left_panel, right_panel));
    let _ = std::fs::remove_dir_all(&dir);
}
