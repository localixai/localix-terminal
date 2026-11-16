//! A tab's zoom in the window: ⇧⌘↩ draws the pane in use alone over the tab's part at once (the
//! other pane leaves it, the side lists the tab as one row marked with what is put by, and the
//! bar's crumb says so too); ⌘D while zoomed asks for a split, which the layout draws whole
//! again; and a terminal an agent opens beside a pane is drawn in its tab's split while the pane
//! shown stays the person's.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use llxterm::browser::{Arranged, Arranging, Browser, Drawn, Heard, Panel};
use llxterm::layout::Tree;

#[track_caller]
fn wait(mut ready: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(Instant::now() < until, "UI did not settle");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The layout of project `zoom`: one tab of `tree`, `focused` in focus, zoomed (`whole`) or not.
fn arranged(tree: &Drawn, focused: &str, whole: bool) -> Vec<Arranged> {
    vec![Arranged {
        window: 0,
        group: "zoom".into(),
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

/// Whether the side's page or the bar's says yes to `script`.
fn side(browser: &Browser, script: &str) -> bool {
    browser
        .peek_tabs(script)
        .unwrap()
        .as_bool()
        .unwrap_or(false)
}
fn bar(browser: &Browser, script: &str) -> bool {
    browser
        .peek_strip(script)
        .unwrap()
        .as_bool()
        .unwrap_or(false)
}

/// The pane the window shows.
fn shown(browser: &Browser) -> Option<String> {
    let listed = browser.listing()?;
    listed.into_iter().find(|tab| tab.shown).map(|tab| tab.id)
}

/// Terminal `id`'s panel in project `zoom`, opened out of sight as the desk opens one beside a
/// pane, loaded.
fn aside(browser: &Browser, id: u32) -> Panel {
    let key = format!("t{id}");
    browser.group(&key, "zoom");
    browser.aside(&key);
    let page = "<html><body style='margin:0'>A pane to zoom</body></html>";
    let panel = browser.panel(id, &format!("Zoom {id}"), page.into());
    wait(|| browser.peek(&panel, "document.readyState") == Ok(json!("complete")));
    panel
}

pub(crate) fn scenario(browser: &Browser) {
    let page = "<html><body style='margin:0'>A pane to zoom</body></html>";
    browser.group("t90", "zoom");
    let one = browser.panel(90, "Zoom 90", page.into());
    let two = aside(browser, 91);
    browser.go("zoom");
    browser.act("t90", false).unwrap();
    let mut tree = Tree::leaf("t90".to_string());
    assert!(tree.split(&"t90".to_string(), "t91".into(), true, true));
    browser.arranged(vec![0], arranged(&tree, "t90", false));
    wait(|| browser.peek(&one, "document.readyState") == Ok(json!("complete")));
    wait(|| {
        side(
            browser,
            "document.querySelectorAll('#sl .pair .half').length === 2",
        )
    });
    let width = |panel: &Panel| browser.peek(panel, "innerWidth").unwrap().as_f64().unwrap();
    wait(|| (width(&one) - width(&two)).abs() < 2.0);
    let half = width(&one);
    let told: Arc<Mutex<Vec<String>>> = Arc::default();
    let heard = Arc::clone(&told);
    browser.listen(move |key, what| {
        let said = if let Heard::Zoomed(on) = what {
            format!("zoom {key} {on}")
        } else if let Heard::SplitNew { beside, side, .. } = what {
            format!("new {beside} {side:?}")
        } else {
            return;
        };
        heard.lock().unwrap().push(said);
    });

    // ⇧⌘↩: the pane in use fills the tab's part at once, before the layout tells it back; the
    // other is out of it, and the side lists the tab as one row.
    browser.click("key zoom");
    wait(|| width(&one) > half * 1.8);
    assert_eq!(told.lock().unwrap().as_slice(), ["zoom t90 true"]);
    wait(|| side(browser, "!document.querySelector('#sl .pair')"));
    // Told back, the row and the crumb say one pane is put by, in words VoiceOver reads.
    browser.arranged(vec![0], arranged(&tree, "t90", true));
    let marked =
        "document.querySelector('#sl .zm')?.getAttribute('aria-label') === 'Zoomed, 1 pane hidden'";
    wait(|| side(browser, marked));
    assert!(side(
        browser,
        "!document.querySelector('#sl [data-key=\"t91\"]')"
    ));
    wait(|| {
        bar(
            browser,
            "document.querySelector('#cr .zm')?.dataset.n === '1'",
        )
    });
    std::thread::sleep(Duration::from_millis(150));
    assert!(width(&one) > half * 1.8, "the layout keeps it zoomed");

    // ⌘D while zoomed asks for a terminal beside it; the layout ends the zoom with the split.
    browser.click("key split right");
    wait(|| {
        told.lock()
            .unwrap()
            .last()
            .is_some_and(|said| said.starts_with("new t90"))
    });
    let three = aside(browser, 92);
    assert!(tree.split(&"t90".to_string(), "t92".into(), true, true));
    browser.arranged(vec![0], arranged(&tree, "t92", false));
    browser.act("t92", false).unwrap();
    wait(|| width(&one) < half * 0.8 && width(&two) > width(&one));
    wait(|| {
        side(
            browser,
            "document.querySelectorAll('#sl .pair .half').length === 3",
        )
    });
    assert!(side(browser, "!document.querySelector('#sl .zm')"));

    // An agent's terminal opened beside t91, under it: in the split, the person's pane shown.
    let four = aside(browser, 93);
    assert!(tree.split(&"t91".to_string(), "t93".into(), false, true));
    browser.arranged(vec![0], arranged(&tree, "t92", false));
    let height = |panel: &Panel| {
        browser
            .peek(panel, "innerHeight")
            .unwrap()
            .as_f64()
            .unwrap()
    };
    wait(|| height(&four) < height(&one) * 0.6 && height(&two) < height(&one) * 0.6);
    wait(|| {
        side(
            browser,
            "[...document.querySelectorAll('#sl .pair .half')].map(h => h.dataset.key).join() === 't90,t92,t91,t93'",
        )
    });
    assert_eq!(shown(browser).as_deref(), Some("t92"));
    browser.listen(|_, _| {});
    drop((one, two, three, four));
}
