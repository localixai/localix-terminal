//! A panel as a tab of the one window, beside the browser's tabs: a terminal session's page as
//! the shell makes it, painted from this thread, read back from its page.

use std::time::{Duration, Instant};

use llx_desk::browser::{Browser, Driver, Panel, WebviewDriver, Window};
use serde_json::{Value, json};

/// What the page shows: the first rows' text, the cursor, the title, and whether it has the
/// keyboard.
const READ: &str = "(() => ({ \
    rows: [0, 1, 2].map(i => term.buffer.active.getLine(i)?.translateToString(true) ?? ''), \
    title: document.title, focus: document.hasFocus(), href: location.href, \
    width: innerWidth, height: innerHeight, visible: document.visibilityState, \
    cols: term.cols, rows_n: term.rows }))()";

/// A script that clears the terminal and writes `lines`, then the end in the title if any.
fn frame(lines: &[String], end: Option<&str>) -> String {
    use base64::Engine;
    let text = format!("\x1b[H\x1b[2J{}", lines.join("\r\n"));
    let write = format!(
        "W(\"{}\")",
        base64::engine::general_purpose::STANDARD.encode(text)
    );
    match end {
        Some(end) => format!("{write};E({})", json!(end)),
        None => write,
    }
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| line.to_string()).collect()
}

/// The tab the window shows, and every tab's id, as the window lists them.
fn showing(browser: &Browser) -> Option<(String, Vec<String>)> {
    let listed = browser.listing()?;
    let shown = listed
        .iter()
        .find(|tab| tab.shown)
        .map(|tab| tab.id.clone());
    Some((
        shown.unwrap_or_default(),
        listed.into_iter().map(|tab| tab.id).collect(),
    ))
}

/// The page once `done` holds of it, within `within`.
fn shown(
    browser: &Browser,
    panel: &Panel,
    within: Duration,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + within;
    loop {
        let read = browser.peek(panel, READ).unwrap_or(Value::Null);
        if done(&read) {
            return read;
        }
        assert!(Instant::now() < deadline, "the panel shows {read:#}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser, driver: &WebviewDriver) {
    let page = llx_desk::terminal::page("session 1 · cat", None);
    let panel = browser.panel(1, "session 1 · cat", page, None);
    let second = Duration::from_secs(1);
    let wait = Duration::from_secs(10);

    // What was written shows in the terminal, and the window never took the keyboard.
    assert!(panel.paint(&frame(&lines(&["hi", "there"]), None)));
    let read = shown(browser, &panel, wait, |read| read["rows"][1] == "there");
    assert_eq!(read["rows"], json!(["hi", "there", ""]));
    assert_eq!(read["focus"], false, "the panel took the keyboard");
    assert_eq!(read["visible"], "visible");
    // The page fills its window (960 by 700 points, less the title bar), and the grid follows.
    let (width, height) = (read["width"].as_f64(), read["height"].as_f64());
    assert!(
        width >= Some(900.0) && height >= Some(600.0),
        "{width:?} by {height:?}"
    );
    assert!(read["cols"].as_u64() > Some(80), "{read}");
    let href = read["href"].clone();

    // A tab beside the browser's, shown as it opens.
    let tab = format!("t{}", panel.id());
    let (shown_now, keys) = showing(browser).expect("the window is open");
    assert_eq!(
        (shown_now.as_str(), keys),
        (tab.as_str(), vec!["w1".into(), tab.clone()])
    );

    // The page goes nowhere else, whatever it is told.
    let _ = browser.peek(&panel, "location.href = 'https://example.com'; 1");
    std::thread::sleep(Duration::from_millis(500));
    let read = shown(browser, &panel, second, |read| read["href"] != Value::Null);
    assert_eq!(read["href"], href, "the panel navigated");

    // A burst: all of it runs in order, and the browser still answers.
    for n in 0..1000 {
        panel.paint(&frame(&lines(&[&n.to_string()]), None));
    }
    shown(browser, &panel, second, |read| read["rows"][0] == "999");
    driver.observe().expect("the browser answers after a burst");

    // A session's end, in the title.
    panel.paint(&frame(&lines(&["bye"]), Some(" · exited 3")));
    shown(browser, &panel, second, |read| {
        read["title"] == "session 1 · cat · exited 3"
    });

    // What the agent does in the browser is shown; a click on the strip shows the terminal again.
    let until_shown = |want: &str| {
        let deadline = Instant::now() + wait;
        while showing(browser).map(|s| s.0).as_deref() != Some(want) {
            assert!(Instant::now() < deadline, "{:?}", showing(browser));
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let _ = driver.navigate("about:blank");
    until_shown("w1");
    browser.click(&format!("show {tab}"));
    until_shown(&tab);
    // The agent shows a tab too, and is told when there is none.
    browser.act("w1", false).unwrap();
    until_shown("w1");
    let listed = browser.listing().unwrap();
    assert_eq!(
        (listed[0].kind.as_str(), listed[0].acted_on),
        ("browser", true)
    );
    assert_eq!(listed[1].kind, "terminal");
    assert!(listed[1].title.starts_with("session 1 · cat"), "{listed:?}");
    let missing = browser.act("t999", false).unwrap_err();
    assert!(
        missing.starts_with("there is no tab t999: the tabs are w1, t"),
        "{missing}"
    );
    browser.act(&tab, false).unwrap();
    until_shown(&tab);
    // Junk from the strip does nothing.
    browser.click("show t999");
    browser.click("<script>");
    until_shown(&tab);
    // Its × closes the tab: the caller hears so, and so does whoever listens to the window.
    let heard = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let told = std::sync::Arc::clone(&heard);
    browser.listen(move |key, what| {
        told.lock()
            .unwrap()
            .push((key.to_string(), format!("{what:?}")))
    });
    browser.click(&format!("close {tab}"));
    until_shown("w1");
    assert_eq!(showing(browser).map(|s| s.1), Some(vec!["w1".to_string()]));
    let heard = heard.lock().unwrap().clone();
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].0, tab);
    assert!(heard[0].1.contains("its program runs on"), "{heard:?}");
    // Out of sight, its screen stays live: what the program prints still reaches it.
    assert!(panel.paint(&frame(&lines(&["after"]), None)));
    shown(browser, &panel, second, |read| read["rows"][0] == "after");
    // The agent shows it again.
    browser.act(&tab, false).unwrap();
    until_shown(&tab);

    // Gone, and the browser carries on.
    drop(panel);
    std::thread::sleep(Duration::from_millis(200));
    driver
        .observe()
        .expect("the browser answers after a panel closed");
}

/// The shown browser window: glass on macOS behind its titlebar, its page the viewport exactly,
/// and its pages opaque: a clear page and a white one make the same screenshot.
pub(crate) fn glass(browser: &Browser, driver: &WebviewDriver, window: &Window, origin: &str) {
    driver.close();
    let shown = Window {
        url: format!("{origin}/blank-white.html"),
        headless: false,
        ..window.clone()
    };
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    // A page there runs its animation frames, as in any shown window.
    let state = driver.evaluate("document.visibilityState").unwrap();
    assert!(
        state.contains("visible") && !state.contains("hidden"),
        "{state}"
    );
    // Coated as a panel is, so its titlebar and toolbar are not the bare material's grey: in
    // glass on macOS, opaque where the person (or a CI runner) reduces transparency.
    let glass = (1, 1, false, true);
    let opaque = (0, -1, true, false);
    let dressed = browser.dressed().map(|d| (d.0, d.1, d.2, d.4));
    let looks = if cfg!(target_os = "macos") {
        vec![Some(glass), Some(opaque)]
    } else {
        vec![Some(opaque)]
    };
    assert!(looks.contains(&dressed), "the browser's frame: {dressed:?}");
    // Under an X server with no window manager (xvfb on a CI runner) GTK gives the view its size
    // a moment after the page loaded: it is read once the view has one, each try a short backoff.
    let mut size = driver.observe().unwrap().viewport;
    for _ in 0..30 {
        if size.h > 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        size = driver.observe().unwrap().viewport;
    }
    // A shown window fits the person's screen (960 to 1440 wide, its height), the strip of tabs above it.
    assert!(
        (960..=1440).contains(&size.w) && size.h >= 600,
        "{} by {}",
        size.w,
        size.h
    );
    if !cfg!(target_os = "macos") {
        // The glass and a page's picture of it are macOS's alone.
        driver.close();
        return;
    }
    let white = driver.screenshot(None).unwrap();
    driver
        .navigate(&format!("{origin}/blank-transparent.html"))
        .unwrap();
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    assert_eq!(driver.observe().unwrap().title, "clear");
    let clear = driver.screenshot(None).unwrap();
    assert!(
        clear.jpeg == white.jpeg,
        "a clear page shows what is behind it"
    );
    driver.close();
}

/// A panel opened before any browser window draws as a shown page does, with animation frames
/// (an emulator draws in them): WebKit must not take it for a hidden one.
pub(crate) fn first(browser: &Browser) {
    let page = llx_desk::terminal::page("session 0 · sh", None);
    let panel = browser.panel(9, "session 0 · sh", page, None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while browser.peek(&panel, "typeof W") != Ok(json!("function")) {
        assert!(Instant::now() < deadline, "the panel never loaded");
        std::thread::sleep(Duration::from_millis(50));
    }
    // The window opens for it alone: its one tab, shown.
    let tab = format!("t{}", panel.id());
    assert_eq!(showing(browser), Some((tab.clone(), vec![tab.clone()])));
    // Long enough for WebKit to have settled what it thinks of the window.
    std::thread::sleep(Duration::from_millis(300));
    // A window that never takes the keyboard never makes llx-desk the active app either: the person's
    // full-screen terminal keeps its keys while a test, `exec` or the agent opens one.
    #[cfg(target_os = "macos")]
    for _ in 0..20 {
        assert!(
            !active(),
            "the process became the active app: it took the person's keys"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let count = "window.seen = 0; (function f() { window.seen++; requestAnimationFrame(f); })(); 1";
    browser.peek(&panel, count).unwrap();
    let read = "[window.seen, document.visibilityState]";
    std::thread::sleep(Duration::from_millis(500));
    let seen = browser.peek(&panel, read).unwrap();
    assert_eq!(seen[1], "visible", "{seen}");
    assert!(seen[0].as_u64() > Some(5), "no animation frames: {seen}");
    // Two projects' tabs: a row of the projects, set as text; one project's, no row.
    let labels = |want: Value| {
        // Each pill: the project's name, then how many of its tabs are open.
        let read = "[...document.querySelectorAll('.p')].map(p => p.firstChild.textContent)";
        let deadline = Instant::now() + Duration::from_secs(10);
        while browser.peek_strip(read) != Ok(want.clone()) {
            assert!(Instant::now() < deadline, "{:?}", browser.peek_strip(read));
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    browser.group(&tab, "alpha");
    browser.group("t8", "beta <i>");
    let other = browser.panel(
        8,
        "session 8 · sh",
        llx_desk::terminal::page("sh", None),
        None,
    );
    labels(json!(["alpha", "beta <i>"]));
    // Under the projects, the shown one's tabs alone.
    let own = "document.querySelectorAll('.t').length";
    let deadline = Instant::now() + Duration::from_secs(10);
    while browser.peek_strip(own) != Ok(json!(1)) {
        assert!(Instant::now() < deadline, "{:?}", browser.peek_strip(own));
        std::thread::sleep(Duration::from_millis(20));
    }
    let listed = browser.listing().unwrap_or_default();
    let groups: Vec<&str> = listed.iter().map(|tab| tab.group.as_str()).collect();
    assert_eq!(groups, ["alpha", "beta <i>"]);
    // Closed by the person, a terminal is in the history menu, still running; a click there makes
    // it a tab again, its screen as live as before.
    browser.click("close t8");
    labels(json!([]));
    browser.click("history");
    let at = menu_entry(browser, "t8", "hidden");
    browser.click(&format!("visit {at}"));
    labels(json!(["alpha", "beta <i>"]));
    assert!(other.paint(&frame(&lines(&["back"]), None)));
    shown(browser, &other, Duration::from_secs(10), |read| {
        read["rows"][0] == "back"
    });
    drop(other);
    labels(json!([]));
    history_menu(browser);
    // The last tab gone, the window is too.
    drop(panel);
    let deadline = Instant::now() + Duration::from_secs(5);
    while showing(browser).is_some() {
        assert!(Instant::now() < deadline, "the window stayed");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The strip's buttons and its history menu: a click opens it over the window, another (the
/// button's, or outside the list) closes it, and a click on a tab closes it too.
fn history_menu(browser: &Browser) {
    let said = |script: &str, want: Value| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while browser.peek_strip(script) != Ok(want.clone()) {
            assert!(
                Instant::now() < deadline,
                "{script}: {:?}",
                browser.peek_strip(script)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let menu = "[document.getElementById('menu').style.display, document.querySelector('#menu .v[data-key=\"t8\"]')?.dataset.state ?? null]";
    said(
        "[bk.title, fw.title, hs.title]",
        json!(["Back", "Forward", "History"]),
    );
    // The terminal that ended is there, its last screen kept.
    browser.click("history");
    said(menu, json!(["block", "ended"]));
    browser.click("history");
    said(menu, json!(["none", null]));
    browser.click("history");
    said(menu, json!(["block", "ended"]));
    browser.click("back");
    said(menu, json!(["none", null]));
    // A click on it shows that screen, which is no tab: none is shown, and the agent's are as they
    // were.
    browser.click("history");
    said(menu, json!(["block", "ended"]));
    let at = menu_entry(browser, "t8", "ended");
    let before = browser.listing().unwrap_or_default();
    browser.click(&format!("visit {at}"));
    said(menu, json!(["none", null]));
    let deadline = Instant::now() + Duration::from_secs(10);
    while browser
        .listing()
        .unwrap_or_default()
        .iter()
        .any(|tab| tab.shown)
    {
        assert!(Instant::now() < deadline, "{:?}", browser.listing());
        std::thread::sleep(Duration::from_millis(20));
    }
    let after = browser.listing().unwrap_or_default();
    let ids = |listed: &[llx_desk::WindowTab]| -> Vec<String> {
        listed.iter().map(|tab| tab.id.clone()).collect()
    };
    assert_eq!(ids(&after), ids(&before));
    let back = before.iter().find(|tab| tab.shown).unwrap().id.clone();
    browser.click(&format!("show {back}"));
}

/// Entry `key` of the history menu once it says `state`: its number.
fn menu_entry(browser: &Browser, key: &str, state: &str) -> u64 {
    let row = format!("document.querySelector('#menu .v[data-key=\"{key}\"]')");
    let read = format!("{row} && [{row}.dataset.state, +{row}.dataset.at]");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let read = browser.peek_strip(&read).unwrap_or(Value::Null);
        if read[0] == state {
            return read[1].as_u64().unwrap();
        }
        assert!(Instant::now() < deadline, "{key} in the menu: {read}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether this process is the app the person's keys go to.
#[cfg(target_os = "macos")]
pub(crate) fn active() -> bool {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    let class = AnyClass::get(c"NSRunningApplication").unwrap();
    // SAFETY: `currentApplication` is a class method that takes nothing and returns this
    // process's NSRunningApplication, which lives as long as the process.
    let app: *mut AnyObject = unsafe { msg_send![class, currentApplication] };
    // SAFETY: `isActive` is a BOOL property of the object above, readable from any thread.
    let active: Bool = unsafe { msg_send![app, isActive] };
    active.as_bool()
}
