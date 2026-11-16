//! The modes a program set (focus reports, the keypad, autowrap, the cursor's shape, the scroll
//! region) as the real xterm panel holds them, before a page is sent the whole screen again and
//! after: a page that reset has them back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Panel};
use llxterm::terminal::{DEFAULT_WAIT, Session, Until};
use serde_json::{Value, json};

/// The modes as the page's emulator holds them.
const MODES: &str = r"(() => {
    const m = term.modes, d = term._core.coreService.decPrivateModes, b = term._core.buffer;
    return { focus: m.sendFocusMode, keypad: m.applicationKeypadMode, wrap: m.wraparoundMode,
        insert: m.insertMode, shape: d.cursorStyle ?? null, blink: d.cursorBlink ?? null,
        region: [b.scrollTop, b.scrollBottom] };
})()";

pub(crate) fn check(browser: &Browser, panel: &Arc<Panel>) {
    let draw = Arc::clone(panel);
    let session = Session::start(
        r"printf '\033[H\033[2J\033[?1004h\033=\033[?7l\033[4h\033[5 q\033[2;6r\033[4;1HMODES-READY'
        sleep 20",
        &std::env::temp_dir(),
        None,
        Some(Box::new(move |script| draw.paint(&script))),
    )
    .unwrap();
    assert_eq!(
        session.wait(Some("MODES-READY"), 0, DEFAULT_WAIT),
        Until::Matched
    );
    let set = json!({
        "focus": true, "keypad": true, "wrap": false, "insert": true,
        "shape": "bar", "blink": true, "region": [1, 5]
    });
    held(browser, panel, &set, "as the program set them");
    // A page that reset (a reload's) has lost them.
    reset(browser, panel);
    // Sent the whole screen again, as a page behind is, it has them back.
    session.behind();
    held(
        browser,
        panel,
        &set,
        "after the page was sent the screen again",
    );
    // The panel is left as it was found, for the scenes after: no focus reports, no keypad.
    drop(session);
    reset(browser, panel);
}

/// The page reset, as a reload's is, and waited for.
fn reset(browser: &Browser, panel: &Arc<Panel>) {
    panel.paint("R('')");
    let deadline = Instant::now() + DEFAULT_WAIT;
    while browser.peek(panel, MODES).map(|now| now["focus"].clone()) != Ok(json!(false)) {
        assert!(Instant::now() < deadline, "the page did not reset");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until the page holds `modes`.
fn held(browser: &Browser, panel: &Arc<Panel>, modes: &Value, when: &str) {
    let deadline = Instant::now() + DEFAULT_WAIT;
    loop {
        let read = browser.peek(panel, MODES);
        if read.as_ref() == Ok(modes) {
            return;
        }
        assert!(Instant::now() < deadline, "{when}: the modes are {read:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
