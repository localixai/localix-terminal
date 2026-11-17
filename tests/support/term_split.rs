//! Terminals drawn by the window in a split tab: two views side by side, each half the tab's
//! width and the grid its own pane's; ⇧⌘↩ zooms the one in use over the whole part (its grid as
//! wide as the tab, the other out of sight with no picture kept) and the split comes back with
//! both views drawn again.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::{Arranged, Arranging, Browser, Drawn, Panel};
use llxterm::layout::Tree;
use llxterm::surface::Surface;
use llxterm::terminal::Session;

use super::terminal_view::seen_in;

/// The terminals' ids, in project `term-split`: apart from the other scenes' and the desk's.
const LEFT: u32 = 912;
const RIGHT: u32 = 913;

/// The layout's word for `tree` in `project`, `focused` in use, zoomed on it when `whole`.
pub(crate) fn arranged_in(
    project: &str,
    tree: &Drawn,
    focused: &str,
    whole: bool,
) -> Vec<Arranged> {
    vec![Arranged {
        window: 0,
        group: project.into(),
        shown: Some(focused.into()),
        tabs: vec![Arranging {
            all: tree.panes(),
            tree: tree.clone(),
            focused: Some(focused.into()),
            docs: vec![],
            whole,
        }],
    }]
}

fn arranged(tree: &Drawn, focused: &str, whole: bool) -> Vec<Arranged> {
    arranged_in("term-split", tree, focused, whole)
}

/// Terminal `id` in project `term-split`, opened beside the pane shown (`aside`) or not.
fn opened(browser: &Browser, id: u32, aside: bool) -> (Arc<Session>, Panel) {
    let command = format!("printf 'pane {id}\\n'; exec sleep 60");
    opened_in(browser, ("term-split", id), aside, &command)
}

/// Terminal `id` of `project` running `command`, opened beside the pane shown (`aside`) or not.
pub(crate) fn opened_in(
    browser: &Browser,
    (project, id): (&str, u32),
    aside: bool,
    command: &str,
) -> (Arc<Session>, Panel) {
    let key = format!("t{id}");
    browser.group(&key, project);
    if aside {
        browser.aside(&key);
    }
    let session = Arc::new(Session::start(command, &std::env::temp_dir(), None, None).unwrap());
    let surface: Arc<dyn Surface> = Arc::clone(&session) as _;
    let panel = browser.term(id, command, surface);
    let wake = panel.waker();
    session.on_changed(move || {
        let _ = wake();
    });
    (session, panel)
}

fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    let (left, left_panel) = opened(browser, LEFT, false);
    browser.go("term-split");
    browser.act(&format!("t{LEFT}"), false).unwrap();
    let alone = seen_in(browser, LEFT, |seen| seen.shown && seen.columns > 0);
    let (right, right_panel) = opened(browser, RIGHT, true);
    let mut tree = Tree::leaf(format!("t{LEFT}"));
    assert!(tree.split(&format!("t{LEFT}"), format!("t{RIGHT}"), true, true));
    let focused = format!("t{LEFT}");
    browser.arranged(vec![0], arranged(&tree, &focused, false));

    // Side by side: each view shown, each grid about half the tab's, its own text in each.
    let half = |id: u32| {
        seen_in(browser, id, |seen| {
            seen.shown && seen.columns < alone.columns * 6 / 10 && seen.columns > 10
        })
    };
    let (one, two) = (half(LEFT), half(RIGHT));
    assert!(one.columns.abs_diff(two.columns) <= 2, "{one:#?} {two:#?}");
    seen_in(browser, RIGHT, |seen| {
        seen.rows
            .iter()
            .any(|row| row.trim() == format!("pane {RIGHT}"))
    });

    // Zoomed: the pane in use as wide as the tab, the other out of sight with nothing kept.
    browser.click("key zoom");
    browser.arranged(vec![0], arranged(&tree, &focused, true));
    seen_in(browser, LEFT, |seen| seen.columns > one.columns * 18 / 10);
    let hidden = seen_in(browser, RIGHT, |seen| !seen.shown);
    assert_eq!(hidden.bytes, 0, "{hidden:#?}");

    // Unzoomed: the split again, both views drawn.
    browser.click("key zoom");
    browser.arranged(vec![0], arranged(&tree, &focused, false));
    until("both panes drawn again", || {
        let read = |id: u32| seen_in(browser, id, |_| true);
        let (a, b) = (read(LEFT), read(RIGHT));
        a.shown && b.shown && b.bytes > 0 && a.columns.abs_diff(one.columns) <= 1
    });
    left.close();
    right.close();
    drop((left_panel, right_panel));
}
