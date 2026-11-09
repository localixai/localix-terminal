//! The window's three states, through real windows (unseen, as every test's): a shown window
//! opens as the window, it is put away only when the person asks and comes back only when they
//! open the app, the agent's acts never change which, putting it away never reflows the page nor
//! stops it drawing, and llx-desk is never the active app.

use std::time::{Duration, Instant};

use llx_desk::browser::{Browser, Driver, WebviewDriver, Window};
use llx_desk::view::Action;
use serde_json::json;

/// Until `want` holds, within ten seconds.
fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The page's frame count over `span`: it rises while the window keeps drawing.
fn frames(driver: &WebviewDriver, span: Duration) -> (u64, u64) {
    let count = "window.seen = 0; (function f() { window.seen++; requestAnimationFrame(f); })(); 1";
    driver.evaluate(count).unwrap();
    let read = || driver.evaluate("window.seen").unwrap().parse().unwrap();
    let first = read();
    std::thread::sleep(span);
    (first, read())
}

/// The page's size as the agent reads it.
fn viewport(driver: &WebviewDriver) -> (u32, u32) {
    let size = driver.observe().unwrap().viewport;
    (size.w, size.h)
}

pub(crate) fn scenario(browser: &Browser, window: &Window, origin: &str) {
    let shown = Window {
        url: format!("{origin}/slow/light.html"),
        headless: false,
        ..window.clone()
    };
    // The window the scenarios before left: no tab in it, as they closed theirs.
    let empty = browser.listing().is_some_and(|tabs| tabs.is_empty());
    assert!(empty, "nothing is open yet");
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();

    // The first tab opens as the window; nothing the agent does puts it away.
    until("the window", || browser.presented() == Some("expanded"));
    browser.act("w1", false).unwrap();
    assert_eq!(browser.presented(), Some("expanded"));
    let size = viewport(&driver);
    assert!(size.0 >= 600 && size.1 >= 400, "{size:?}");
    // The person puts it away (the yellow button, ⌘M): out of sight, it keeps drawing.
    browser.click("collapse");
    until("the window put away", || {
        browser.presented() == Some("away")
    });
    let (first, later) = frames(&driver, Duration::from_millis(500));
    assert!(
        later > first + 5,
        "the page stopped drawing: {first} to {later}"
    );
    assert_eq!(viewport(&driver), size, "putting it away resized the page");

    // Nothing the agent does brings it back: it shows a tab, it goes to another page.
    browser.act("w1", false).unwrap();
    assert_eq!(browser.presented(), Some("away"));
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    assert_eq!(browser.presented(), Some("away"));

    // The person opens the app (its Dock icon): the window is back, the page as big as it was.
    browser.present();
    until("the window back", || {
        browser.presented() == Some("expanded")
    });
    assert_eq!(viewport(&driver), size, "bringing it back resized the page");
    browser.act("w1", false).unwrap();
    assert_eq!(browser.presented(), Some("expanded"));

    // Whose turn: the person comments, the browser's toolbar says the agent is paused; an action
    // of the agent's waits, and it says so; putting the window away ends commenting and lets
    // it go.
    let turn = "document.getElementById('tn').textContent";
    browser.click("comment");
    until("the strip says the agent is paused", || {
        browser.peek_nav(turn) == Ok(json!("Paused while you comment"))
    });
    let page = driver.observe().unwrap();
    let mover = driver.clone();
    let (sender, done) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = sender.send(mover.act(&page.epoch, &Action::ScrollDown));
    });
    until("the strip says the agent waits", || {
        browser.peek_nav(turn) == Ok(json!("Agent is waiting for you"))
    });
    assert!(done.try_recv().is_err(), "the scroll waits for the person");
    browser.click("collapse");
    until("the window away once more", || {
        browser.presented() == Some("away")
    });
    let went = done.recv_timeout(Duration::from_secs(5));
    assert!(
        matches!(went, Ok(Ok(()))),
        "put away, commenting ends and the scroll goes: {went:?}"
    );
    until("the strip shows no turn", || {
        browser.peek_nav(turn) == Ok(json!(""))
    });

    // A screenshot is the same whichever state the window is in.
    driver
        .screenshot(None)
        .expect("a picture while the window is away");

    // The last tab closed, the window stays, with no tab.
    driver.close();
    until("no tab left", || {
        browser.listing().is_some_and(|tabs| tabs.is_empty())
    });
    assert_ne!(browser.presented(), Some("hidden"), "the window stays");
    // As the person leaves it: the window on screen, for the scenarios after.
    browser.present();
    until("the window again", || {
        browser.presented() == Some("expanded")
    });
}

/// Whether this process is the app the person's keys go to, polled while `during` runs.
#[cfg(target_os = "macos")]
pub(crate) fn never_active(during: impl FnOnce()) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = AtomicBool::new(false);
    let seen = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !stop.load(Ordering::SeqCst) {
                if super::panel::active() {
                    seen.store(true, Ordering::SeqCst);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        during();
        stop.store(true, Ordering::SeqCst);
    });
    assert!(
        !seen.load(Ordering::SeqCst),
        "the process became the active app: it took the person's keys"
    );
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn never_active(during: impl FnOnce()) {
    during();
}
