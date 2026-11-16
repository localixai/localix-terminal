//! Find in a web page in a real shown window, driven as the person drives it: the window's Find,
//! Find Next and Find Previous (the clicks the Edit menu's items fire), what is typed into the
//! browser toolbar's find row, Return and Shift+Return, the count it says, the match shown
//! scrolled to, a new document of the tab searched again, and Esc, which shuts the row and gives
//! the keys back to the page. The page, whose policy allows it no style of its own, is never
//! changed: its marks are highlights.

use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Driver, Window};
use serde_json::Value;

/// What the toolbar's row shows, and whether its field has the keys.
const READ: &str = "(() => ({ open: !document.getElementById('find').hidden, \
    count: document.getElementById('find-n').textContent, \
    query: document.getElementById('find-q').value, \
    field: document.activeElement === document.getElementById('find-q') }))()";

/// The marks in the page: every match lit, the one shown, the sheets adopted and the rules of
/// the marks' own.
const MARKS: &str = "[CSS.highlights.get('llxterm-find')?.size ?? 0, \
    CSS.highlights.get('llxterm-find-shown')?.size ?? 0, document.adoptedStyleSheets.length, \
    document.adoptedStyleSheets[0]?.cssRules.length ?? 0].join()";

/// Until `want` holds, within ten seconds.
fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Typed into the row's field, as a person types it.
fn typed(query: &str) -> String {
    format!(
        "(() => {{ const f = document.getElementById('find-q'); f.value = {query:?}; \
         f.dispatchEvent(new Event('input')); return 1 }})()"
    )
}

/// A key pressed in the row's field.
fn key(name: &str, shift: bool) -> String {
    format!(
        "(() => {{ document.getElementById('find-q').dispatchEvent(new KeyboardEvent('keydown', \
         {{ key: {name:?}, shiftKey: {shift}, bubbles: true, cancelable: true }})); return 1 }})()"
    )
}

pub(crate) fn scenario(browser: &Browser, window: &Window, origin: &str) {
    let shown = Window {
        url: format!("{origin}/find.html"),
        headless: false,
        ..window.clone()
    };
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    let page = |script: &str| driver.evaluate(script).unwrap();
    let nav = |script: &str| browser.peek_nav(script).unwrap();
    let row = || nav(READ);
    let counted = |want: &str| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let read = row();
            if read["count"] == want {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the row says {read:#}, not {want}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let before = page("document.body.innerHTML");

    // Find: the toolbar's row opens, the keys in its field.
    browser.click("key find text");
    until("the row open with the keys in its field", || {
        let read = row();
        read["open"] == Value::Bool(true) && read["field"] == Value::Bool(true)
    });
    until("the toolbar holds the window's keys", || {
        browser.keys().as_deref() == Some("nav")
    });

    // Typed, every match a person sees is found, case ignored; the hidden ones are not.
    nav(&typed("needle"));
    counted("1 of 3");
    browser.click("key find next");
    counted("2 of 3");
    browser.click("key find next");
    counted("3 of 3");
    // The last one is far down the page: it was brought into view.
    until("the last match scrolled to", || {
        page("scrollY").parse::<f64>().unwrap_or(0.0) > 100.0
    });
    browser.click("key find previous");
    counted("2 of 3");
    nav(&key("Enter", false));
    counted("3 of 3");
    nav(&key("Enter", true));
    counted("2 of 3");
    // From the last, the next is the first.
    browser.click("key find next");
    browser.click("key find next");
    counted("1 of 3");

    // The marks are highlights, their look a sheet the page adopted though its policy allows
    // it no style; the page's own markup is as it was.
    assert_eq!(page(MARKS), "3,1,1,2");
    assert_eq!(page("document.body.innerHTML"), before);

    // Another query; one with none found says so.
    nav(&typed("haystack"));
    counted("1 of 1");
    nav(&typed("haystacks"));
    counted("No results");
    nav(&typed("needle"));
    until("the query found again", || {
        row()["count"]
            .as_str()
            .is_some_and(|count| count.ends_with(" of 3"))
    });

    // A new document in the tab (the same text, from an address that takes its time): its row
    // stays open and finds in it again.
    driver
        .navigate(&format!("{origin}/slow/find.html"))
        .unwrap();
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    until("the new document lit", || page(MARKS) == "3,1,1,2");
    until("the row open on it", || {
        let read = row();
        read["open"] == Value::Bool(true)
            && read["query"] == "needle"
            && read["count"]
                .as_str()
                .is_some_and(|count| count.ends_with(" of 3"))
    });

    // Esc: the row shuts, the marks go, and the page has the keys again.
    browser.click("key find text");
    until("the field has the keys again", || {
        row()["field"] == Value::Bool(true)
    });
    nav(&key("Escape", false));
    until("the row shut", || row()["open"] == Value::Bool(false));
    until("the marks gone", || page(MARKS) == "0,0,0,0");
    until("the page holds the keys", || {
        browser.keys().is_some_and(|held| held.starts_with('w'))
    });
    assert_eq!(page("document.body.innerHTML"), before);

    // As the scenarios after expect it: no tab left, the window on screen.
    driver.close();
    until("no tab left", || {
        browser.listing().is_some_and(|tabs| tabs.is_empty())
    });
    browser.present();
}
