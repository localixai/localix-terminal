//! Native UI regressions: a split draws independently, titles do no layout work, and IME stays put.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use llx_desk::browser::{Arranged, Arranging, Browser, Heard};

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

fn arranging(shares: Vec<f64>) -> Vec<Arranged> {
    vec![Arranged {
        window: 0,
        group: "ui".into(),
        shown: Some("t70".into()),
        tabs: vec![Arranging {
            panes: vec!["t70".into(), "t71".into()],
            across: true,
            shares,
            focused: Some("t70".into()),
        }],
    }]
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
    browser.arranged(vec![0], arranging(vec![0.35, 0.65]));
    let width = |id| browser.peek(id, "innerWidth").unwrap().as_f64().unwrap();
    wait(|| width(&one) < width(&two) * 0.7);
    let before = width(&one);
    let portion = Arc::new(Mutex::new(None));
    let heard = Arc::clone(&portion);
    browser.listen(move |_, what| {
        if let Heard::SplitResized(value) = what {
            *heard.lock().unwrap() = Some(value);
        }
    });
    browser.click("split-move 80 done");
    wait(|| portion.lock().unwrap().is_some());
    wait(|| width(&one) > before + 60.0);
    let after = width(&one);
    browser.arranged(vec![0], arranging(vec![0.35, 0.65]));
    wait(|| {
        tabs(
            browser,
            "document.querySelectorAll('#sl .pair').length === 1",
        )
    });
    std::thread::sleep(Duration::from_millis(180));
    assert!((width(&one) - after).abs() < 2.0);
    let saved = f64::from(portion.lock().unwrap().unwrap()) / 10_000.0;
    browser.arranged(vec![0], arranging(vec![saved, 1.0 - saved]));
    browser.listen(|_, _| {});
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

fn board(browser: &Browser) {
    let mut state = llx_desk::board::Board::default();
    state.add("First", "", None, "person").unwrap();
    browser.group("b72", "ui");
    let panel = browser.panel(
        llx_desk::browser::BOARD + 72,
        "UI",
        llx_desk::board::page("UI"),
    );
    browser.go("ui");
    browser.act("b72", false).unwrap();
    wait(|| browser.peek(&panel, "typeof B") == Ok(json!("function")));
    panel.paint(&llx_desk::board::frame(&state, "b72", "UI", 1, &json!([])));
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
    panel.paint(&llx_desk::board::frame(&state, "b72", "UI", 2, &json!([])));
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
    panel.paint(&llx_desk::board::frame(&state, "b72", "UI", 3, &json!([])));
    wait(|| browser.peek(&panel, "board.now === 3").unwrap() == json!(true));
    assert_eq!(
        browser
            .peek(&panel, "document.querySelector('.card') === __same")
            .unwrap(),
        json!(true)
    );
}
