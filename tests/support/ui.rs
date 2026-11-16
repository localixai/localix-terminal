//! Native UI regressions: a split tab is drawn from its tree, a divider's final move survives
//! an older layout, a tab of three panes is laid out and resized on both axes, the Pane menu's
//! keys split, move the keys, move a divider, make the tab even and close a pane, titles do no
//! layout work, and IME stays put.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use llxterm::browser::{Arranged, Arranging, Asked, Browser, Drawn, Heard, Side};
use llxterm::layout::Tree;

#[track_caller]
fn wait(mut ready: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(Instant::now() < until, "UI did not settle");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn tabs(browser: &Browser, script: &str) -> bool {
    browser
        .peek_tabs(script)
        .unwrap()
        .as_bool()
        .unwrap_or(false)
}

/// The layout of project `ui` as the desk tells it: one tab of `tree`, `focused` in focus.
fn arranged(tree: Drawn, focused: &str) -> Vec<Arranged> {
    vec![Arranged {
        window: 0,
        group: "ui".into(),
        shown: Some(focused.into()),
        tabs: vec![Arranging {
            all: tree.panes(),
            tree,
            focused: Some(focused.into()),
            docs: vec![],
            whole: false,
        }],
    }]
}

/// `t70` beside `t71`, the first `share` of the width.
fn pair(share: f64) -> Vec<Arranged> {
    let mut tree = Tree::leaf("t70".to_string());
    assert!(tree.split(&"t70".to_string(), "t71".into(), true, true));
    assert!(tree.resize(&[], 1, share));
    arranged(tree, "t70")
}

/// Every divider move the window tells the layout, in order: its branch, the divider, and the
/// share in ten-thousandths.
type Moves = Arc<Mutex<Vec<(Vec<usize>, usize, u16)>>>;

fn moves(browser: &Browser) -> Moves {
    let moves: Moves = Arc::default();
    let heard = Arc::clone(&moves);
    browser.listen(move |_, what| {
        if let Heard::Resized { path, at, share } = what {
            heard.lock().unwrap().push((path.to_vec(), at, share));
        }
    });
    moves
}

pub(crate) fn scenario(browser: &Browser) {
    let page = "<html><body style='margin:0'>A quiet pane</body></html>";
    browser.group("t70", "ui");
    let one = browser.panel(70, "Smooth one", page.into());
    browser.group("t71", "ui");
    let two = browser.panel(71, "Smooth two", page.into());
    browser.go("ui");
    browser.act("t70", false).unwrap();
    wait(|| {
        tabs(
            browser,
            "document.querySelectorAll('#sl [data-key]').length === 2",
        )
    });
    // Split View draws the split at once, before any layout tells it back.
    browser.click("key split");
    wait(|| {
        tabs(
            browser,
            "document.querySelectorAll('#sl .pair .half').length === 2",
        )
    });

    // Updating text must preserve the actual row and do no FLIP measurements.
    browser
        .peek_tabs(
            r#"
        window.__row = document.querySelector('[data-key="t70"]');
        window.__rects = 0;
        window.__getRect = Element.prototype.getBoundingClientRect;
        Element.prototype.getBoundingClientRect = function() {
            if (this.closest('#sl')) window.__rects++;
            return __getRect.call(this);
        };
        true
    "#,
        )
        .unwrap();
    one.paint("document.title = 'A calmer title'");
    wait(|| tabs(browser, "__row.title.includes('A calmer title')"));
    assert!(tabs(browser, "__row.isConnected && __rects === 0"));
    browser
        .peek_tabs("Element.prototype.getBoundingClientRect = __getRect; true")
        .unwrap();

    // A saved size is restored, and a late old snapshot cannot undo a divider's final move.
    browser.arranged(vec![0], pair(0.35));
    let width = |id| browser.peek(id, "innerWidth").unwrap().as_f64().unwrap();
    wait(|| width(&one) < width(&two) * 0.7);
    let before = width(&one);
    let moved = moves(browser);
    browser.click("split-move 0 80 done");
    wait(|| !moved.lock().unwrap().is_empty());
    wait(|| width(&one) > before + 60.0);
    let after = width(&one);
    browser.arranged(vec![0], pair(0.35));
    wait(|| {
        tabs(
            browser,
            "document.querySelectorAll('#sl .pair').length === 1",
        )
    });
    std::thread::sleep(Duration::from_millis(180));
    assert!((width(&one) - after).abs() < 2.0);
    let (path, at, share) = moved.lock().unwrap()[0].clone();
    assert_eq!(
        (path.as_slice(), at),
        (&[][..], 1),
        "the root's one divider"
    );
    let saved = f64::from(share) / 10_000.0;
    browser.arranged(vec![0], pair(saved));
    browser.listen(|_, _| {});
    // Split View again: whole at once, the layout told after.
    browser.click("key split");
    wait(|| {
        tabs(
            browser,
            "document.querySelectorAll('#sl .pair').length === 0",
        )
    });
    drop((one, two));

    board(browser);
}

/// A tab of three panes, made as the desk makes it (one split beside, another under): `t73` on
/// the left, `t74` over `t75` on the right.
fn three_tree() -> Drawn {
    let (left, top, under) = ("t73".to_string(), "t74".to_string(), "t75".to_string());
    let mut tree = Tree::leaf(left.clone());
    assert!(tree.split(&left, top.clone(), true, true));
    assert!(tree.split(&top, under, false, true));
    tree
}

pub(crate) fn three(browser: &Browser) {
    let page = "<html><body style='margin:0'>One of three</body></html>";
    let panels: Vec<_> = (73..76)
        .map(|id| {
            browser.group(&format!("t{id}"), "ui");
            browser.panel(id, &format!("Pane {id}"), page.into())
        })
        .collect();
    browser.go("ui");
    browser.act("t73", false).unwrap();
    browser.arranged(vec![0], arranged(three_tree(), "t73"));
    // Each pane's page is there to be asked before it is measured.
    for panel in &panels {
        wait(|| browser.peek(panel, "document.readyState") == Ok(json!("complete")));
    }
    // The side lists the three as one row, in the order they lie.
    wait(|| {
        tabs(
            browser,
            "[...document.querySelectorAll('#sl .pair .half')].map(h => h.dataset.key).join() === 't73,t74,t75'",
        )
    });
    let size = |at: usize, what: &str| browser.peek(&panels[at], what).unwrap().as_f64().unwrap();
    // Laid out by the tree: the left as tall as the column, the column's two halves of it.
    wait(|| size(1, "innerHeight") < size(0, "innerHeight") * 0.6);
    assert!((size(0, "innerWidth") - size(1, "innerWidth")).abs() < 2.0);
    assert!((size(1, "innerWidth") - size(2, "innerWidth")).abs() < 0.5);
    assert!((size(1, "innerHeight") - size(2, "innerHeight")).abs() < 2.0);
    let moved = moves(browser);
    // The column's divider (the tree's second) dragged down: the upper pane grows.
    let (high, wide) = (size(1, "innerHeight"), size(0, "innerWidth"));
    browser.click("split-move 1 60 done");
    wait(|| size(1, "innerHeight") > high + 50.0);
    assert!(
        (size(0, "innerWidth") - wide).abs() < 1.0,
        "the other way stays"
    );
    // The root's divider dragged left: the left pane narrows, the column widens.
    browser.click("split-move 0 -80 done");
    wait(|| size(0, "innerWidth") < wide - 70.0);
    assert!(
        size(1, "innerHeight") > high + 50.0,
        "the first divider stays where it was let go"
    );
    wait(|| moved.lock().unwrap().len() == 2);
    let told = moved.lock().unwrap().clone();
    assert_eq!((told[0].0.as_slice(), told[0].1), (&[1][..], 1), "{told:?}");
    assert!(told[0].2 > 5_000);
    assert_eq!((told[1].0.as_slice(), told[1].1), (&[][..], 1), "{told:?}");
    assert!(told[1].2 < 5_000);
    // The pane shown is the one in focus: the layout hears it.
    let focused = Arc::new(Mutex::new(None));
    let heard = Arc::clone(&focused);
    browser.listen(move |key, what| {
        if what == Heard::Focused {
            *heard.lock().unwrap() = Some(key.to_string());
        }
    });
    browser.act("t75", false).unwrap();
    wait(|| focused.lock().unwrap().as_deref() == Some("t75"));
    browser.listen(|_, _| {});
    drop(panels);
}

/// What the window tells the layout while the Pane menu's keys are pressed, each as words.
fn told(browser: &Browser) -> Arc<Mutex<Vec<String>>> {
    let told: Arc<Mutex<Vec<String>>> = Arc::default();
    let heard = Arc::clone(&told);
    browser.listen(move |key, what| {
        let said = if let Heard::SplitNew {
            beside,
            side,
            asked,
        } = &what
        {
            format!("new {beside} {side:?} {asked:?}")
        } else if let Heard::Resized { path, at, share } = what {
            format!("resized {path:?} {at} {share}")
        } else if what == Heard::Equalized {
            format!("even {key}")
        } else if matches!(what, Heard::Closed(_)) {
            format!("closed {key}")
        } else {
            return;
        };
        heard.lock().unwrap().push(said);
    });
    told
}

/// The pane the window shows.
fn shown(browser: &Browser) -> Option<String> {
    let listed = browser.listing()?;
    listed.into_iter().find(|tab| tab.shown).map(|tab| tab.id)
}

/// A new terminal the desk opens on `side` of `beside` as it does (out of sight, then in its
/// tab's tree, then shown): its panel.
fn opened(
    browser: &Browser,
    id: u32,
    tree: &mut Drawn,
    (beside, side): (&str, Side),
) -> llxterm::browser::Panel {
    let page = "<html><body style='margin:0'>A pane of keys</body></html>";
    let key = format!("t{id}");
    browser.group(&key, "ui");
    browser.aside(&key);
    let panel = browser.panel(id, &format!("Keys {id}"), page.into());
    assert!(tree.split(
        &beside.to_string(),
        key.clone(),
        side.across(),
        side.forward()
    ));
    browser.arranged(vec![0], arranged(tree.clone(), &key));
    browser.act(&key, false).unwrap();
    wait(|| browser.peek(&panel, "document.readyState") == Ok(json!("complete")));
    panel
}

pub(crate) fn keys(browser: &Browser) {
    let page = "<html><body style='margin:0'>A pane of keys</body></html>";
    browser.group("t80", "ui");
    let first = browser.panel(80, "Keys 80", page.into());
    browser.go("ui");
    browser.act("t80", false).unwrap();
    let mut tree = Tree::leaf("t80".to_string());
    browser.arranged(vec![0], arranged(tree.clone(), "t80"));
    wait(|| browser.peek(&first, "document.readyState") == Ok(json!("complete")));
    let told = told(browser);
    let last = |told: &Arc<Mutex<Vec<String>>>| told.lock().unwrap().last().cloned();
    // ⌘D: the window asks for a terminal on the right of the pane in use, in its folder.
    browser.click("key split right");
    wait(|| last(&told).is_some());
    let asked = format!("new t80 {:?} {:?}", Side::Right, Asked::Terminal(None));
    assert_eq!(last(&told), Some(asked));
    let second = opened(browser, 81, &mut tree, ("t80", Side::Right));
    // ⇧⌘D from the new one: a terminal under it.
    browser.click("key split down");
    let asked = format!("new t81 {:?} {:?}", Side::Down, Asked::Terminal(None));
    wait(|| last(&told).as_ref() == Some(&asked));
    let third = opened(browser, 82, &mut tree, ("t81", Side::Down));
    let panels = [&first, &second, &third];
    let size = |at: usize, what: &str| browser.peek(panels[at], what).unwrap().as_f64().unwrap();
    // Three panes: the first as tall as the tab, the other two one above the other beside it.
    wait(|| size(1, "innerHeight") < size(0, "innerHeight") * 0.6);
    assert!((size(1, "innerWidth") - size(2, "innerWidth")).abs() < 0.5);
    wait(|| {
        tabs(
            browser,
            "[...document.querySelectorAll('#sl .pair .half')].map(h => h.dataset.key).join() === 't80,t81,t82'",
        )
    });
    // ⌥⌘←: the keys to the pane on the left; ⌥⌘→ back to the one of two used last.
    browser.click("key pane left");
    wait(|| shown(browser).as_deref() == Some("t80"));
    browser.click("key pane right");
    wait(|| shown(browser).as_deref() == Some("t82"));
    // ⌃⌘→ on the right edge's pane: the divider on its left goes right, the first pane widens.
    let wide = size(0, "innerWidth");
    browser.click("key resize right");
    wait(|| last(&told).is_some_and(|said| said.starts_with("resized [] 1 ")));
    wait(|| size(0, "innerWidth") > wide + 20.0);
    // ⌃⌘=: the tab even again, at once, and the layout told.
    browser.click("key equalize");
    wait(|| last(&told).as_deref() == Some("even t82"));
    wait(|| (size(0, "innerWidth") - size(1, "innerWidth")).abs() < 2.0);
    // ⌘W: the pane in use closes, and the one used last of the rest takes the keys.
    browser.click("key close");
    wait(|| told.lock().unwrap().iter().any(|said| said == "closed t82"));
    wait(|| shown(browser).as_deref() == Some("t80"));
    browser.listen(|_, _| {});
}

fn board(browser: &Browser) {
    let mut state = llxterm::board::Board::default();
    state.add("First", "", None, "person").unwrap();
    browser.group("b72", "ui");
    let panel = browser.panel(
        llxterm::browser::BOARD + 72,
        "UI",
        llxterm::board::page("UI"),
    );
    browser.go("ui");
    browser.act("b72", false).unwrap();
    wait(|| browser.peek(&panel, "typeof B") == Ok(json!("function")));
    panel.paint(&llxterm::board::frame(&state, "b72", "UI", 1, &json!([])));
    wait(|| {
        browser
            .peek(&panel, "document.querySelectorAll('.card').length === 1")
            .unwrap()
            == json!(true)
    });
    browser
        .peek(
            &panel,
            r#"
        document.querySelector('.card').click();
        window.__card = document.querySelector('.card');
        window.__field = document.querySelector('.title');
        __field.value = 'My draft';
        __field.focus();
        __field.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true}));
        true
    "#,
        )
        .unwrap();
    state.cards[0].title = "Changed elsewhere".to_string();
    panel.paint(&llxterm::board::frame(&state, "b72", "UI", 2, &json!([])));
    wait(|| browser.peek(&panel, "board.now === 2").unwrap() == json!(true));
    assert_eq!(
        browser
            .peek(
                &panel,
                "__field.isConnected && document.querySelector('.card') === __card"
            )
            .unwrap(),
        json!(true)
    );
    browser
        .peek(
            &panel,
            "__field.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true})); true",
        )
        .unwrap();
    wait(|| {
        browser
            .peek(&panel, "document.querySelector('.title') !== __field")
            .unwrap()
            == json!(true)
    });
    assert_eq!(
        browser
            .peek(&panel, "document.activeElement.value")
            .unwrap(),
        json!("My draft")
    );

    // Clock-only frames do not rebuild the board.
    browser
        .peek(
            &panel,
            "window.__same = document.querySelector('.card'); true",
        )
        .unwrap();
    panel.paint(&llxterm::board::frame(&state, "b72", "UI", 3, &json!([])));
    wait(|| browser.peek(&panel, "board.now === 3").unwrap() == json!(true));
    assert_eq!(
        browser
            .peek(&panel, "document.querySelector('.card') === __same")
            .unwrap(),
        json!(true)
    );
}
