//! Find in a terminal, driven as the person drives it: the window's Find, Find Next and Find
//! Previous (the clicks the Edit menu's items fire), what is typed into the find row under the
//! terminal (the browser toolbar's row, alone there), the count it says, the matches lit in the
//! view, case and Cyrillic folded alike, a match in the history found and scrolled to, the
//! count kept up as new output comes while the row is open, and Esc,
//! which shuts the row, takes the marks away and gives the keys back to the terminal.

use std::time::{Duration, Instant};

use llxterm::browser::Browser;
use llxterm::surface::Surface;
use serde_json::Value;

use super::terminal_view::{opened, seen_in};

/// The terminal's id: apart from the other scenes' and the desk's.
const ID: u32 = 901;

/// What the row shows, whether the toolbar shows it alone, and whether its field has the keys.
const READ: &str = "(() => ({ open: !document.getElementById('find').hidden, \
    term: document.documentElement.hasAttribute('data-term'), \
    count: document.getElementById('find-n').textContent, \
    field: document.activeElement === document.getElementById('find-q') }))()";

/// Typed into the row's field, as a person types it.
fn typed(query: &str) -> String {
    format!(
        "(() => {{ const f = document.getElementById('find-q'); f.value = {query:?}; \
         f.dispatchEvent(new Event('input')); return 1 }})()"
    )
}

/// Esc pressed in the row's field.
const ESCAPE: &str = "(() => { document.getElementById('find-q').dispatchEvent(new \
    KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })); return 1 })()";

/// Until `want` holds, within ten seconds.
fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    // Three greetings, the first two far up in the history, the last at the foot of the screen.
    let session = opened(
        browser,
        ID,
        "printf 'Привет one\\nпривет two\\n'; seq 1 200; printf 'ПРИВЕТ last\\n'; \
         stty -echo; read a; printf 'привет again\\nпривет more\\n'; exec sleep 60",
    );
    seen_in(browser, ID, |seen| {
        seen.rows.iter().any(|row| row.trim() == "ПРИВЕТ last")
    });
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

    // Find: the row opens under the terminal, alone, the keys in its field.
    browser.click("key find text");
    until("the row open under the terminal with the keys", || {
        let read = row();
        read["open"] == Value::Bool(true)
            && read["term"] == Value::Bool(true)
            && read["field"] == Value::Bool(true)
    });
    until("the toolbar holds the keys", || {
        browser.keys().as_deref() == Some("nav")
    });

    // Typed in another case, every greeting is found; the one shown is the newest, lit in view.
    nav(&typed("привет"));
    counted("3 of 3");
    seen_in(browser, ID, |seen| seen.found >= 1);
    // The next is the one before it, far up the history: the view scrolls to it.
    browser.click("key find next");
    counted("2 of 3");
    seen_in(browser, ID, |seen| {
        seen.found >= 1 && seen.rows.iter().any(|row| row.trim() == "привет two")
    });
    browser.click("key find previous");
    counted("3 of 3");
    // New output while the row is open: its matches are counted as it comes, the one shown
    // kept, nothing typed in the row.
    Surface::text(&*session, "\n");
    counted("3 of 5");
    seen_in(browser, ID, |seen| {
        seen.rows.iter().any(|row| row.trim() == "привет more")
    });
    nav(&typed("прощай"));
    counted("No results");
    seen_in(browser, ID, |seen| seen.found == 0);

    // Esc: the row shuts, the marks go, and the terminal has the keys again.
    nav(&typed("привет"));
    counted("5 of 5");
    nav(ESCAPE);
    until("the row shut", || row()["open"] == Value::Bool(false));
    seen_in(browser, ID, |seen| seen.found == 0);
    until("the terminal holds the keys", || {
        browser.keys().as_deref() == Some(&format!("t{ID}"))
    });
    session.close();
}
