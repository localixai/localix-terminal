//! The native driver against fixture pages on a local server, in a real headless window.
//! The event loop must own the main thread, so this test has its own `main` (`harness = false`).
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "test code: a panic fails the test; its own `main` prints the harness's lines and \
              exits with the run's code; a scenario is one window's checks, read in order"
)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use llx_desk::browser::{self, Driver, Observation, WebviewDriver, Window};
use llx_desk::view::{Action, ViewError};

#[path = "support/board.rs"]
mod board;
#[path = "support/panel.rs"]
mod panel;
#[cfg(target_os = "macos")]
#[path = "support/presence.rs"]
mod presence;
#[path = "support/refs.rs"]
mod refs;
#[path = "support/terminal_colors.rs"]
mod terminal_colors;
#[path = "support/told.rs"]
mod told;
#[cfg(target_os = "macos")]
#[path = "support/ui.rs"]
mod ui;

/// Serves `tests/fixtures` on an ephemeral port; returns its origin. Each connection has its own
/// thread: a browser opens connections ahead of need, and one that never sends a request must not
/// hold up the next page.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(&stream));
        }
    });
    origin
}

/// One request: the fixture it names, after the delay its path asks for.
fn answer(stream: &std::net::TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .trim_start_matches('/');
    // A server that takes its time: a navigation must be followed, not raced.
    let delayed = [("slow/", 800), ("slower/", 3000), ("slowest/", 6000)];
    let (path, delay) = (delayed.iter())
        .find_map(|(prefix, ms)| Some((path.strip_prefix(prefix)?, *ms)))
        .unwrap_or((path, 0));
    std::thread::sleep(Duration::from_millis(delay));
    let file = format!("{}/tests/fixtures/{path}", env!("CARGO_MANIFEST_DIR"));
    let (status, body) = match std::fs::read(&file) {
        Ok(body) => ("200 OK", body),
        // An archive: a file to keep, which no page shows.
        Err(_) if path.ends_with(".zip") => ("200 OK", b"PK\x03\x04".to_vec()),
        Err(_) => ("404 Not Found", b"missing".to_vec()),
    };
    let kind = match path.ends_with(".zip") {
        true => "application/zip",
        false => "text/html; charset=utf-8",
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = stream;
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(&body));
}

fn find<'a>(page: &'a Observation, role: &str, name: &str) -> &'a str {
    page.elements
        .iter()
        .find(|element| element.role == role && element.name == name)
        .unwrap_or_else(|| panic!("no {role} \"{name}\" in {:#?}", page.elements))
        .reference
        .as_str()
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
fn act(driver: &WebviewDriver, page: &Observation, action: Action) {
    driver
        .act(&page.epoch, &action)
        .unwrap_or_else(|error| panic!("{action:?}: {error}"));
}

fn scenario(driver: &WebviewDriver, origin: &str) {
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Book");
    let accept = find(&page, "button", "Accept");
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == accept && e.dialog),
        "Accept is in the dialog"
    );
    let later = page.elements.iter().find(|e| e.name == "Later").unwrap();
    assert!(later.disabled(), "a disabled control reads as disabled");

    // A disabled control refuses; a click closes the dialog.
    let refused = driver.act(
        &page.epoch,
        &Action::Click {
            reference: later.reference.clone(),
        },
    );
    assert!(matches!(refused, Err(ViewError::Refused(_))), "{refused:?}");
    act(
        driver,
        &page,
        Action::Click {
            reference: accept.to_string(),
        },
    );
    let page = driver.observe().unwrap();
    assert!(
        page.elements.iter().all(|e| e.name != "Accept"),
        "the dialog closed"
    );

    // Typing, picking an option and Enter reach the form's own submit handler.
    let city = find(&page, "textbox", "City").to_string();
    act(
        driver,
        &page,
        Action::Type {
            reference: city.clone(),
            text: "Zurich".into(),
        },
    );
    act(
        driver,
        &page,
        Action::Select {
            reference: find(&page, "combobox", "Class").into(),
            value: "Business".into(),
        },
    );
    act(
        driver,
        &page,
        Action::PressEnter {
            reference: Some(city),
        },
    );
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "sent:Zurich:Business");
    let field = page.elements.iter().find(|e| e.name == "City").unwrap();
    assert_eq!(field.value.as_deref(), Some("Zurich"));

    // An unknown ref is stale; a covered control refuses rather than clicking what is on top.
    let unknown = driver.act(
        &page.epoch,
        &Action::Type {
            reference: "e999".into(),
            text: "x".into(),
        },
    );
    assert!(matches!(unknown, Err(ViewError::Stale(_))), "{unknown:?}");
    let veiled = find(&page, "button", "Under the veil").to_string();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Raise veil").into(),
        },
    );
    let covered = driver.act(&page.epoch, &Action::Click { reference: veiled });
    assert!(
        matches!(&covered, Err(ViewError::Refused(m)) if m.starts_with("covered")),
        "{covered:?}"
    );
    assert_ne!(driver.observe().unwrap().title, "veiled clicked");

    // Picking the chosen option again fires no change: the page's handler (as on
    // the-internet.herokuapp.com/dropdown) would otherwise reset the selection.
    driver
        .navigate(
            &std::env::var("BROWSER_DROPDOWN")
                .unwrap_or_else(|_| format!("{origin}/dropdown.html")),
        )
        .unwrap();
    for _ in 0..3 {
        let page = driver.observe().unwrap();
        let reference = page.elements[0].reference.clone();
        act(
            driver,
            &page,
            Action::Select {
                reference,
                value: "2".into(),
            },
        );
        let after = driver.observe().unwrap();
        assert_eq!(
            after.elements[0].value.as_deref(),
            Some("Option 2"),
            "{:?}",
            after.elements[0]
        );
    }

    // An address the system cannot read is refused, and the window lives on: WebKit's load of
    // one panics on the main thread, which ended the whole process.
    assert!(driver.navigate("https:// playsnake.org").is_err());
    assert!(driver.add_tab("https:// playsnake.org").is_err());

    // A link inside a heading is a control in the outline, not only the heading's words.
    driver.navigate(&format!("{origin}/results.html")).unwrap();
    let page = driver.observe().unwrap();
    let title = find(&page, "link", "Cheap flights to Georgia");
    assert!(
        page.outline
            .contains(&format!("link \"Cheap flights to Georgia\" [ref={title}]"))
            && page.outline.contains("heading 2 \"Plain heading\""),
        "{}",
        page.outline
    );

    // Keys reach the focused element (here the body) with the key, code and keyCode games read.
    driver.navigate(&format!("{origin}/keys.html")).unwrap();
    driver
        .press(&["Up".into(), "ArrowLeft".into(), "a".into()], 0)
        .unwrap();
    assert_eq!(
        driver.evaluate("document.title").unwrap(),
        "keys:ArrowUp/ArrowUp/38;ArrowLeft/ArrowLeft/37;a/KeyA/65;"
    );
    // A script presses keys the same way, with `press` (a game bot's loop); a script may still
    // declare a `press` of its own.
    driver.navigate(&format!("{origin}/keys.html")).unwrap();
    assert_eq!(
        driver
            .evaluate("press('ArrowDown'); await sleep(10); press('b'); document.title")
            .unwrap(),
        "keys:ArrowDown/ArrowDown/40;b/KeyB/66;"
    );
    assert_eq!(driver.evaluate("const press = 2; press * 3").unwrap(), "6");
    // A change anywhere in the document, in sight or not, moves its revision; a look moves none.
    let seen = driver.observe().unwrap().revision;
    assert_eq!(driver.observe().unwrap().revision, seen);
    driver
        .evaluate("document.title = 'keys:'; document.body.append('out of sight')")
        .unwrap();
    assert!(driver.observe().unwrap().revision > seen);
    // A browser's own reload keys reload the page, as a person's would; the keys after them reach
    // the page loaded again.
    driver.press(&["a".into(), "Ctrl+R".into()], 0).unwrap();
    assert_eq!(driver.evaluate("document.title").unwrap(), "keys:");
    driver.press(&["F5".into(), "b".into()], 0).unwrap();
    assert_eq!(
        driver.evaluate("document.title").unwrap(),
        "keys:b/KeyB/66;"
    );

    // In a field, character keys type at the caret, as a person's would, with the `input` the page
    // hears; Backspace erases, Ctrl+A selects all so the next key replaces it. A key the page
    // cancels types nothing.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    driver
        .evaluate("const c = document.getElementById('city'); c.focus(); c.addEventListener('input', () => document.title = 'heard:' + c.value)")
        .unwrap();
    let typed: Vec<String> = ["f", "e", "r", "n", "x", "Backspace"]
        .map(String::from)
        .into();
    driver.press(&typed, 0).unwrap();
    assert_eq!(driver.evaluate("document.title").unwrap(), "heard:fern");
    driver
        .press(
            &[
                "Ctrl+A".into(),
                "R".into(),
                "o".into(),
                "m".into(),
                "e".into(),
            ],
            0,
        )
        .unwrap();
    assert_eq!(driver.evaluate("city.value").unwrap(), "Rome");
    driver
        .evaluate(
            "city.addEventListener('keydown', e => { if (e.key === 'z') e.preventDefault() })",
        )
        .unwrap();
    driver.press(&["z".into()], 0).unwrap();
    assert_eq!(driver.evaluate("city.value").unwrap(), "Rome");

    // Calendar days carry their date or month: "5" alone is ambiguous across two months.
    driver.navigate(&format!("{origin}/calendar.html")).unwrap();
    let days: Vec<String> = driver
        .observe()
        .unwrap()
        .elements
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(days, ["5 (2026-12-05)", "5 €120 (January 2027)"]);
    // A modal hides the form from the controls; a password's value never shows.
    driver.navigate(&format!("{origin}/modal.html")).unwrap();
    let page = driver.observe().unwrap();
    let names: Vec<&str> = page.elements.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Done"]);
    assert!(
        !page.outline.contains("secret") && !page.text.contains("secret"),
        "{}",
        page.outline
    );
    driver.navigate(&format!("{origin}/dropdown.html")).unwrap();

    // Scripts: an expression, statements (last value), a body with its own return, a throw.
    assert_eq!(driver.evaluate("document.title").unwrap(), "Dropdown");
    assert_eq!(driver.evaluate("const n = 2; n * 21").unwrap(), "42");
    assert_eq!(
        driver
            .evaluate("let n = 0; while (n < 3) { n++ } n * 2")
            .unwrap(),
        "6"
    );
    assert_eq!(
        driver
            .evaluate("const xs = [1, 2]; return xs.length")
            .unwrap(),
        "2"
    );
    assert_eq!(
        driver.evaluate("await Promise.resolve({a: 1})").unwrap(),
        "{\n  \"a\": 1\n}"
    );
    let thrown = driver.evaluate("null.x");
    assert!(
        matches!(&thrown, Err(ViewError::Refused(m)) if m.starts_with("the script threw")),
        "{thrown:?}"
    );

    // A script whose page leaves answers once the new document loads, not at the script timeout.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    let started = Instant::now();
    let left = driver
        .evaluate("location.href = '/second.html'; await new Promise((r) => setTimeout(r, 30000))")
        .unwrap();
    assert!(left.contains("new document"), "{left}");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(driver.observe().unwrap().title, "Second");

    // Navigations only the Navigation API reports are still followed to their load.
    for button in ["Submit by script", "Go later"] {
        driver.navigate(&format!("{origin}/leave.html")).unwrap();
        let page = driver.observe().unwrap();
        act(
            driver,
            &page,
            Action::Click {
                reference: find(&page, "button", button).into(),
            },
        );
        assert_eq!(driver.observe().unwrap().title, "Second", "{button}");
    }

    // A parsed page is used after a short grace: a slow image (an ad, a video) does not hold the
    // run until it arrives, nor makes the page busy.
    let started = Instant::now();
    driver.navigate(&format!("{origin}/heavy.html")).unwrap();
    let page = driver.observe().unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(2000),
        "{:?}",
        started.elapsed()
    );
    assert!(page.title == "Heavy" && !page.busy, "{page:?}");
    std::thread::sleep(Duration::from_millis(2500));

    // An action waits for the request it started (results in 0.8 s); one that takes far longer
    // than that wait (6 s) leaves the page busy until the answer arrives.
    driver.navigate(&format!("{origin}/search.html")).unwrap();
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Search").into(),
        },
    );
    let page = driver.observe().unwrap();
    assert!(!page.busy && page.text.contains("3 results"), "{page:?}");
    let clicked = Instant::now();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Search everywhere").into(),
        },
    );
    let page = driver.observe().unwrap();
    // Busy while the request is out and young: a click that waited its turn behind the other
    // scenarios (the person commenting) may come back after that.
    if clicked.elapsed() < Duration::from_millis(4500) {
        assert!(page.busy && !page.text.contains("9 results"), "{page:?}");
    }
    // Its answer comes in the end, and the page is not busy then.
    let answered = Instant::now() + Duration::from_secs(10);
    let page = loop {
        let page = driver.observe().unwrap();
        if page.text.contains("9 results") || Instant::now() > answered {
            break page;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    assert!(!page.busy && page.text.contains("9 results"), "{page:?}");

    // A banner in a same-origin frame over the page: its controls are offered as a dialog, and
    // typing and clicking reach them; the page under it refuses until it is gone.
    driver.navigate(&format!("{origin}/consent.html")).unwrap();
    let page = driver.observe().unwrap();
    let accept = find(&page, "button", "Accept").to_string();
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == accept && e.dialog),
        "{:#?}",
        page.elements
    );
    assert!(
        page.text.starts_with("We value your privacy"),
        "{}",
        page.text
    );
    let radio = find(&page, "radio", "<script>").to_string();
    let covered = driver.act(
        &page.epoch,
        &Action::Click {
            reference: radio.clone(),
        },
    );
    assert!(
        matches!(&covered, Err(ViewError::Refused(m)) if m.starts_with("covered")),
        "{covered:?}"
    );
    act(
        driver,
        &page,
        Action::Type {
            reference: find(&page, "textbox", "Mail").into(),
            text: "a@b.c".into(),
        },
    );
    act(driver, &page, Action::Click { reference: accept });
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Quiz:a@b.c");
    act(driver, &page, Action::Click { reference: radio });
    let page = driver.observe().unwrap();
    let checked = page.elements.iter().find(|e| e.name == "<script>").unwrap();
    assert!(checked.states.contains(&"checked".into()), "{checked:?}");

    // A link navigates; the old epoch no longer acts on the new document.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    let page = driver.observe().unwrap();
    let old = page.epoch.clone();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "link", "Next page").into(),
        },
    );
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Second");
    let back = find(&page, "link", "Back").to_string();
    let stale = driver.act(&old, &Action::Click { reference: back });
    assert!(matches!(stale, Err(ViewError::Stale(_))), "{stale:?}");

    tabs(driver, origin);
    never_came(driver, origin);
    not_shown(driver, origin);
    screenshots(driver, origin);
    rerendered(driver, origin);
    drifted(driver, origin);

    // A page mounted under a `display: contents` root (a framework's): its content is offered,
    // though the root has no box of its own.
    driver.navigate(&format!("{origin}/contents.html")).unwrap();
    let page = driver.observe().unwrap();
    assert!(
        page.outline.contains("Mounted by script"),
        "{}",
        page.outline
    );
    find(&page, "button", "Get started");
    find(&page, "link", "Docs");

    // A dialog in an open shadow root (a consent web component): its controls are offered, as a
    // dialog; what it covers names it; its button closes it.
    driver.navigate(&format!("{origin}/shadow.html")).unwrap();
    let page = driver.observe().unwrap();
    let save = find(&page, "button", "Save + Exit").to_string();
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == save && e.dialog),
        "{:#?}",
        page.elements
    );
    let covered = driver.act(
        &page.epoch,
        &Action::Click {
            reference: find(&page, "button", "New Game").into(),
        },
    );
    assert!(
        matches!(&covered, Err(ViewError::Refused(m))
            if m.starts_with("covered: a dialog \"Consent to Cookies\"")),
        "{covered:?}"
    );
    act(driver, &page, Action::Click { reference: save });
    let page = driver.observe().unwrap();
    assert!(page.elements.iter().all(|e| !e.dialog), "the dialog closed");
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "New Game").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "new game");

    // Controls a page made from plain elements (a pointer cursor, an onclick) are offered and
    // clicked like buttons; a pointer group once, and a wrapper around a link not at all.
    driver.navigate(&format!("{origin}/levels.html")).unwrap();
    let page = driver.observe().unwrap();
    let names: Vec<(&str, &str)> = page
        .elements
        .iter()
        .map(|e| (e.role.as_str(), e.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("button", "Slug"),
            ("button", "Worm"),
            ("button", "Inline"),
            ("button", "Card title"),
            ("link", "More"),
            ("button", "sound toggle"),
        ]
    );
    let worm = find(&page, "button", "Worm").to_string();
    assert!(
        page.outline
            .contains(&format!("button \"Worm\" [ref={worm}]"))
            && page.outline.contains("text \"Plain words\""),
        "{}",
        page.outline
    );
    act(driver, &page, Action::Click { reference: worm });
    assert_eq!(driver.observe().unwrap().title, "level:Worm");

    // The agent closes the window: it is closed, and the task goes on (closing it by hand would
    // end this test with the window-closed exit code instead of "driver: ok").
    driver.close();
    let closing = Instant::now();
    while driver.is_open() && closing.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!driver.is_open(), "the window closed");
    assert!(matches!(driver.observe(), Err(ViewError::Unavailable(_))));
}

/// A second page on the one open window (another client of the server): it opens a tab of its own
/// beside the first page's, and the first page's tab is left as it was.
/// Two calls that open the window at once: the second is refused, and the window stays open to
/// every driver and to the next page that would open it.
fn opened_twice(browser: &browser::Browser, driver: &WebviewDriver, window: &Window) {
    assert!(browser.open(window.clone()).is_err());
    assert!(
        driver.is_open(),
        "the window is open, but its drivers see it closed"
    );
    assert!(browser.attach().is_some(), "an open window is attached to");
}

fn second_page(browser: &browser::Browser, driver: &WebviewDriver, origin: &str) {
    let (before, _) = driver.tabs();
    let page = browser::Page::new(browser.clone(), true);
    page.open(&format!("{origin}/form.html"))
        .expect("a second page opens beside the first");
    let (after, _) = driver.tabs();
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    for tab in after
        .iter()
        .filter(|t| !before.iter().any(|b| b.id == t.id))
    {
        driver
            .close_tab(tab.id)
            .expect("the second page's tab closes");
    }
    let (left, _) = driver.tabs();
    assert_eq!(left.len(), before.len(), "{left:?}");
}

/// A page loading a slow address still answers other threads at once (no
/// lock held across a load): whether it is open, its driver, a tab of its own.
fn answers_while_loading(browser: &browser::Browser, driver: &WebviewDriver, origin: &str) {
    let (before, _) = driver.tabs();
    let page = browser::Page::new(browser.clone(), true);
    std::thread::scope(|scope| {
        let opening = scope.spawn(|| page.open(&format!("{origin}/slower/form.html")));
        // Its tab is made at once; its document comes 3 s later.
        let asked = Instant::now();
        while driver.tabs().0.len() == before.len() && asked.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let asked = Instant::now();
        let _ = (page.is_open(), page.driver().is_ok());
        let _ = page.pinned(1);
        let waited = asked.elapsed();
        assert!(
            waited < Duration::from_secs(1),
            "the page waited on the load: {waited:?}"
        );
        opening.join().unwrap().expect("the slow page opens");
    });
    assert!(page.is_open());
    for tab in driver
        .tabs()
        .0
        .iter()
        .filter(|t| !before.iter().any(|b| b.id == t.id))
    {
        driver.close_tab(tab.id).expect("its tab closes");
    }
}

/// A tab is started at once and loads after, so the app's lock on making tabs is never held
/// through a load: a page on the open window starts a slow address (3 s) in a tab of its own and
/// says the tab's id before the document comes; a driver pinned to it waits for it.
fn starts_at_once(browser: &browser::Browser, driver: &WebviewDriver, origin: &str) {
    let page = browser::Page::new(browser.clone(), true);
    let asked = Instant::now();
    let id = (page.start(&format!("{origin}/slower/form.html"))).expect("a tab starts");
    let waited = asked.elapsed();
    assert!(waited < Duration::from_secs(1), "it waited: {waited:?}");
    let tab = page.pinned(id).driver().expect("its driver");
    tab.wait_new();
    assert_eq!(tab.observe().unwrap().title, "Book");
    driver.close_tab(id).expect("its tab closes");
}

/// Visual feedback: the person turns commenting on from the strip; then a press on a button picks
/// it instead of pressing it (a script's click still presses it), the note typed in its card is
/// held, and Send hands the batch to the listener with the element's selector.
fn feedback(browser: &browser::Browser, driver: &WebviewDriver) {
    let (sender, heard) = std::sync::mpsc::channel();
    let sender = std::sync::Mutex::new(sender);
    browser.listen(move |tab, heard| {
        if let browser::Heard::Sent(what) = heard {
            let _ = sender
                .lock()
                .unwrap()
                .send((tab.to_string(), what.to_string()));
        }
    });
    let cursor = |driver: &WebviewDriver| {
        driver
            .evaluate("await sleep(300); document.documentElement.style.cursor")
            .unwrap()
    };
    browser.click("comment");
    assert_eq!(
        cursor(driver),
        "crosshair",
        "the strip's button turns it on"
    );
    let pressed = driver
        .evaluate(
            "window.titled = document.title;
             document.getElementById('veiled').click();
             const pressed = document.title;
             document.title = 'before';
             pressed",
        )
        .unwrap();
    // A check's presses count as the person's, and the overlay's root is open to it: only the
    // native side's call says so, never the page.
    driver.comment_checked(true).unwrap();
    let after = driver
        .evaluate(
            "const button = document.getElementById('veiled');
             const r = button.getBoundingClientRect();
             const at = { clientX: r.left + 5, clientY: r.top + 5, bubbles: true, composed: true, cancelable: true };
             for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click'])
               button.dispatchEvent(new PointerEvent(type, at));
             const card = document.querySelector('desk-feedback').shadowRoot;
             card.querySelector('textarea').value = 'make it red';
             card.querySelector('button.s').click();
             const after = document.title;
             document.title = window.titled;
             after",
        )
        .unwrap();
    let picked = format!("{pressed} | {after}");
    assert_eq!(
        picked, "veiled clicked | before",
        "a script presses, the person picks"
    );
    assert!(
        heard.recv_timeout(Duration::from_millis(500)).is_err(),
        "held until sent"
    );
    browser.click("send");
    let (tab, what) = heard.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(tab.starts_with('w'), "{tab}");
    assert!(what.starts_with("The person sent 1 comment(s)"), "{what}");
    assert!(what.contains(": make it red"), "{what}");
    assert!(what.contains("Selector: #veiled"), "{what}");
    // Its picture, taken from the screen, in a file the agent can read: on macOS and Linux,
    // where the window captures; elsewhere the comment comes without one.
    let picture = what.split("Picture of it: ").nth(1);
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        let file = picture
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default();
        let jpeg = std::fs::read(file).unwrap();
        assert!(jpeg.starts_with(&[0xff, 0xd8]), "a JPEG");
    } else {
        assert!(picture.is_none(), "{what}");
    }
    assert_eq!(cursor(driver), "", "Send ends commenting: the agent's turn");
    browser.click("comment");
    assert_eq!(cursor(driver), "crosshair", "on again");
    browser.click("comment");
    assert_eq!(cursor(driver), "", "and off");
    browser.listen(|_, _| {});
}

/// A modal dialog open (a cart) leaves the rest of the page inert and is drawn above it: the
/// comment card goes inside it, on top, and takes typing; when it closes the overlay comes back.
/// The check turns commenting on from the native side, the overlay's root open to it.
fn feedback_in_a_dialog(driver: &WebviewDriver) {
    driver
        .evaluate(
            "const dialog = document.createElement('dialog');
             dialog.id = 'cart';
             dialog.innerHTML = '<button id=\"inside\">Checkout</button>';
             document.body.append(dialog);
             dialog.showModal();
             true",
        )
        .unwrap();
    driver.comment_checked(true).unwrap();
    let said = driver
        .evaluate(
            "const dialog = document.getElementById('cart');
             const host = document.querySelector('desk-feedback');
             const inside = document.getElementById('inside');
             const r = inside.getBoundingClientRect();
             const at = { clientX: r.left + 5, clientY: r.top + 5, bubbles: true, composed: true, cancelable: true };
             for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click'])
               inside.dispatchEvent(new PointerEvent(type, at));
             const note = host.shadowRoot.querySelector('textarea');
             const typed = note !== null && host.shadowRoot.activeElement === note;
             const placed = host.parentNode === dialog;
             const above = !host.showPopover || host.matches(':popover-open');
             note?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
             const closed = host.shadowRoot.querySelector('textarea') === null && dialog.open;
             dialog.close();
             await sleep(50);
             const back = host.parentNode === document.documentElement;
             JSON.stringify({ typed, placed, above, closed, back })",
        )
        .unwrap();
    driver.comment_checked(false).unwrap();
    driver
        .evaluate("document.getElementById('cart').remove(); true")
        .unwrap();
    assert_eq!(
        said,
        r#"{"typed":true,"placed":true,"above":true,"closed":true,"back":true}"#
    );
}

/// A page of a size of its own: framed as a phone, the page sees the phone's width, and the
/// window is as it was; `window` gives it the whole window again. A size larger than the window
/// is drawn smaller to fit it, and the page still lays out at that size.
fn sizes(driver: &WebviewDriver) {
    let width = |driver: &WebviewDriver| driver.evaluate("innerWidth").unwrap();
    let height = |driver: &WebviewDriver| driver.evaluate("innerHeight").unwrap();
    let whole = width(driver);
    let said = driver.frame("iPhone 15 Pro").unwrap();
    assert!(
        said.starts_with("Page size iPhone 15 Pro: 393 × 852."),
        "{said}"
    );
    assert_eq!(width(driver), "393");
    let said = driver.frame("3000x2000").unwrap();
    assert!(said.starts_with("Page size Custom: 3000 × 2000."), "{said}");
    let near = |seen: String, side: f64| (seen.parse::<f64>().unwrap() - side).abs() <= 1.0;
    assert!(near(width(driver), 3000.0) && near(height(driver), 2000.0));
    assert!(driver.frame("watch").is_err(), "a size it does not know");
    let said = driver.frame("window").unwrap();
    assert!(said.starts_with("The page fills the window"), "{said}");
    assert_eq!(width(driver), whole);
}

/// While the person comments, an action that moves the page waits for them; a script that reads
/// it does not. Their Send gives the turn back, and the action goes.
fn waits_for_the_person(browser: &browser::Browser, driver: &WebviewDriver) {
    let page = driver.observe().unwrap();
    browser.click("comment");
    let mover = driver.clone();
    let (sender, done) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = sender.send(mover.act(&page.epoch, &Action::ScrollDown));
    });
    assert!(
        done.recv_timeout(Duration::from_millis(800)).is_err(),
        "the scroll waits while the person comments"
    );
    assert_eq!(
        driver.evaluate("document.title").unwrap(),
        driver.evaluate("document.title").unwrap(),
        "a script still reads the page"
    );
    browser.click("send");
    let went = done.recv_timeout(Duration::from_secs(5));
    assert!(
        matches!(went, Ok(Ok(()))),
        "Send gives the turn back: {went:?}"
    );
    // Send ended commenting: the next action goes at once.
    let page = driver.observe().unwrap();
    let started = Instant::now();
    driver.act(&page.epoch, &Action::ScrollUp).unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
}

/// Tabs: one opened, switched to and back, one per link with target=_blank and per window.open,
/// a tab out of sight still drawing, and closing them.
fn tabs(driver: &WebviewDriver, origin: &str) {
    let titles = |driver: &WebviewDriver| {
        std::thread::sleep(Duration::from_millis(300));
        let (tabs, active) = driver.tabs();
        let titles: Vec<String> = tabs.iter().map(|tab| tab.title.clone()).collect();
        (titles, active)
    };
    driver.navigate(&format!("{origin}/tabs.html")).unwrap();
    assert_eq!(driver.tabs().0.len(), 1);
    let second = driver.add_tab(&format!("{origin}/form.html")).unwrap();
    driver.on(second).wait_new();
    assert_eq!(
        (second, driver.observe().unwrap().title.as_str()),
        (2, "Book")
    );
    assert_eq!(titles(driver), (vec!["Tabs".into(), "Book".into()], 2));

    // Tab 1 kept drawing while out of sight.
    driver.switch_tab(1).unwrap();
    let drawn = |driver: &WebviewDriver| {
        driver
            .evaluate("window.frames_drawn")
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let before = drawn(driver);
    driver.switch_tab(2).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    driver.switch_tab(1).unwrap();
    // A tab kept drawing draws at the display's pace (a virtual display's is slow: a few frames
    // in 600 ms); one that was throttled draws none.
    let after = drawn(driver);
    assert!(after > before + 2, "{before} then {after}");

    // A driver pinned to a tab acts there, whatever is shown: two tabs' scripts run at once.
    let (one, two) = (driver.on(1), driver.on(2));
    assert_eq!(one.observe().unwrap().title, "Tabs");
    assert_eq!(two.observe().unwrap().title, "Book");
    assert_eq!(driver.tabs().1, 1, "the tab shown stays");
    let started = Instant::now();
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| one.evaluate("await sleep(700); document.title"));
        let b = scope.spawn(|| two.evaluate("await sleep(700); document.title"));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(
        (a.unwrap(), b.unwrap()),
        ("Tabs".to_string(), "Book".to_string())
    );
    let took = started.elapsed();
    assert!(
        took < Duration::from_millis(1300),
        "one after the other: {took:?}"
    );

    // Tabs opened at the same time get ids and pages of their own.
    let (form, second) = std::thread::scope(|scope| {
        let form = scope.spawn(|| driver.add_tab(&format!("{origin}/form.html")).unwrap());
        let second = scope.spawn(|| driver.add_tab(&format!("{origin}/second.html")).unwrap());
        (form.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(form, second);
    for (id, title) in [(form, "Book"), (second, "Second")] {
        let tab = driver.on(id);
        tab.wait_new();
        assert_eq!(tab.observe().unwrap().title, title, "tab {id}");
    }
    assert!(driver.close_tab(form.max(second)).unwrap());
    assert!(driver.close_tab(form.min(second)).unwrap());
    driver.switch_tab(1).unwrap();

    // A link with target=_blank and window.open each open a tab, which is then acted on.
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "link", "Open second").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "Second");
    driver.switch_tab(1).unwrap();
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Open dropdown").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "Dropdown");
    assert_eq!(
        titles(driver),
        (
            vec![
                "Tabs".into(),
                "Book".into(),
                "Second".into(),
                "Dropdown".into()
            ],
            4
        )
    );

    // Closing the tab acted on goes to the one before it; a closed id is refused.
    assert!(driver.close_tab(4).unwrap());
    assert_eq!(driver.observe().unwrap().title, "Second");
    assert!(matches!(driver.switch_tab(4), Err(ViewError::Refused(_))));
    for id in [3, 2] {
        assert!(driver.close_tab(id).unwrap());
    }
    assert_eq!(titles(driver), (vec!["Tabs".into()], 1));
    assert_eq!(driver.observe().unwrap().title, "Tabs");

    // A sign-in popup opens as a tab that keeps its opener: its answer reaches the page that
    // opened it, and its window.close() closes the tab. Elsewhere than macOS it opens as a tab of
    // its own, with no opener (popup.rs), and is closed here.
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Sign in").into(),
        },
    );
    if cfg!(target_os = "macos") {
        let one = driver.on(1);
        let signed = (0..40).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            one.observe().is_ok_and(|page| page.title == "Signed in")
        });
        assert!(signed, "the opener never heard back: {:?}", titles(driver));
        let closed = (0..40).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            driver.tabs().0.len() == 1
        });
        assert!(closed, "the popup's tab stayed: {:?}", titles(driver));
    } else {
        let opened = (0..40).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            driver.tabs().0.len() == 2
        });
        assert!(opened, "the popup opened no tab: {:?}", titles(driver));
        let popup = driver.tabs().0.iter().map(|tab| tab.id).find(|id| *id != 1);
        assert!(driver.close_tab(popup.unwrap()).unwrap());
        driver.switch_tab(1).unwrap();
    }
    driver.evaluate("document.title = 'Tabs'").unwrap();
    // A driver pinned to a tab that is closed, or never was, acts nowhere: it never falls back to
    // the tab shown, where its clicks would land on another page.
    for gone in [2, 9] {
        let pinned = driver.on(gone);
        assert!(
            matches!(pinned.observe(), Err(ViewError::Unavailable(ref why)) if why.contains("closed")),
            "tab {gone}"
        );
        assert!(pinned.evaluate("document.title = 'moved'").is_err());
        assert!(pinned.navigate(&format!("{origin}/second.html")).is_err());
    }
    assert_eq!(driver.observe().unwrap().title, "Tabs");
    // A new tab reuses a closed one's place.
    let reused = driver.add_tab(&format!("{origin}/second.html")).unwrap();
    driver.on(reused).wait_new();
    assert_eq!(reused, 2);
    assert!(driver.close_tab(2).unwrap());
}

/// A port nothing listens at: taken, then let go.
fn free_port() -> u16 {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    taken.local_addr().unwrap().port()
}

/// A new tab asked for a page at a port nothing listens at, once it says so: the tab, the port
/// and the address.
fn dead_tab(driver: &WebviewDriver) -> (u32, u16, String) {
    let port = free_port();
    let dead = format!("http://127.0.0.1:{port}/second.html");
    let tab = driver.add_tab(&dead).unwrap();
    let pinned = driver.on(tab);
    pinned.wait_new();
    let line = (0..40).find_map(|_| {
        std::thread::sleep(Duration::from_millis(100));
        let text = pinned.evaluate("document.body.innerText").ok()?;
        text.contains("Could Not Load").then_some(text)
    });
    let line = line.expect("the tab said nothing of the page that never came");
    assert!(line.contains(&format!("127.0.0.1:{port}")), "{line}");
    (tab, port, dead)
}

/// The fixtures are served at `port` from now on; whether tab `tab` then comes to show `url`.
fn comes_after(driver: &WebviewDriver, tab: u32, port: u16, url: &str, reload: &dyn Fn()) -> bool {
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(&stream));
        }
    });
    reload();
    let pinned = driver.on(tab);
    (0..40).any(|_| {
        std::thread::sleep(Duration::from_millis(100));
        (pinned.evaluate("location.href")).is_ok_and(|href| href.contains(url))
    })
}

/// A page that never comes: the tab says so in one line, keeps the address asked of it, and
/// loads the page once something answers there. A tab that shows a page keeps it.
fn never_came(driver: &WebviewDriver, origin: &str) {
    let (tab, port, dead) = dead_tab(driver);
    let pinned = driver.on(tab);
    let kept = |driver: &WebviewDriver| {
        let (tabs, _) = driver.tabs();
        let tab = tabs.iter().find(|open| open.id == tab).unwrap();
        (tab.url.clone(), tab.title.clone())
    };
    assert_eq!(kept(driver), (dead.clone(), format!("127.0.0.1:{port}")));
    // Something answers there now: Reload, pressed as an agent presses it, brings the page.
    let came = comes_after(driver, tab, port, &dead, &|| {
        pinned.press(&["cmd+r".to_owned()], 0).unwrap();
    });
    assert!(came, "Reload left the tab at {:?}", kept(driver));
    // Asked for a page that answers, the tab is that page's.
    pinned.navigate(&format!("{origin}/form.html")).unwrap();
    assert!(kept(driver).0.ends_with("/form.html"), "{:?}", kept(driver));
    // A tab that shows a page keeps it when the next one never comes.
    let _ = pinned.navigate(&format!("http://127.0.0.1:{}/", free_port()));
    std::thread::sleep(Duration::from_millis(900));
    let text = pinned.evaluate("document.body.innerText").unwrap();
    assert!(!text.contains("Could Not Load"), "{text}");
    assert!(driver.close_tab(tab).unwrap());
    driver.switch_tab(1).unwrap();
}

/// What comes and no page shows (an archive): the tab says so in one line, by the file's name,
/// and keeps the address. A tab that shows a page keeps its page.
fn not_shown(driver: &WebviewDriver, origin: &str) {
    let archive = format!("{origin}/get/build%20all.zip");
    let tab = driver.add_tab(&archive).unwrap();
    let pinned = driver.on(tab);
    pinned.wait_new();
    let line = (0..40).find_map(|_| {
        std::thread::sleep(Duration::from_millis(100));
        let text = pinned.evaluate("document.body.innerText").ok()?;
        text.contains("No Preview").then_some(text)
    });
    let line = line.expect("the tab said nothing of the file no page shows");
    assert!(line.contains("No Preview for build all.zip"), "{line}");
    let (tabs, _) = driver.tabs();
    let open = tabs.iter().find(|open| open.id == tab).unwrap();
    assert_eq!(
        (open.url.as_str(), open.title.as_str()),
        (archive.as_str(), "build all.zip")
    );
    // A page's own link to an archive leaves the page as it is.
    pinned.navigate(&format!("{origin}/form.html")).unwrap();
    let _ = pinned.navigate(&archive);
    std::thread::sleep(Duration::from_millis(900));
    let text = pinned.evaluate("document.body.innerText").unwrap();
    assert!(!text.contains("No Preview"), "{text}");
    assert!(driver.close_tab(tab).unwrap());
    driver.switch_tab(1).unwrap();
}

/// The person's own Reload (the toolbar's, ⌘R) asks a tab for the address it could not load.
fn the_person_reloads(browser: &browser::Browser, driver: &WebviewDriver) {
    let (tab, port, dead) = dead_tab(driver);
    driver.switch_tab(tab).unwrap();
    let came = comes_after(driver, tab, port, &dead, &|| browser.click("reload"));
    assert!(
        came,
        "the person's Reload brought no page: {:?}",
        driver.tabs()
    );
    assert!(driver.close_tab(tab).unwrap());
    driver.switch_tab(1).unwrap();
}

/// A screenshot is the WebView's own picture, as JPEG; a selector crops it to one element.
fn screenshots(driver: &WebviewDriver, origin: &str) {
    driver.navigate(&format!("{origin}/canvas.html")).unwrap();
    let viewport = driver.observe().unwrap().viewport;
    let page = driver.screenshot(None).unwrap();
    assert_eq!(&page.jpeg[..3], [0xFF, 0xD8, 0xFF], "a JPEG");
    let ratio = |w: u32, h: u32| f64::from(w) / f64::from(h);
    assert!(
        (ratio(page.width, page.height) - ratio(viewport.w, viewport.h)).abs() < 0.02
            && page.width.max(page.height) <= 1280
            && page.width >= viewport.w.min(1280) / 2,
        "{}x{} for a {}x{} viewport",
        page.width,
        page.height,
        viewport.w,
        viewport.h
    );
    // Below the fold: scrolled into view, then cut out with its 8 px margin (216x116).
    let board = driver.screenshot(Some("#board")).unwrap();
    assert!(
        (ratio(board.width, board.height) - 216.0 / 116.0).abs() < 0.05,
        "{}x{}",
        board.width,
        board.height
    );
    assert!(board.width < page.width && board.jpeg.len() < page.jpeg.len());
    let missing = driver.screenshot(Some("#nothing"));
    assert!(
        matches!(&missing, Err(ViewError::Refused(m)) if m.contains("nothing on the page matches")),
        "{missing:?}"
    );
}

/// A page that renders its controls again on every click (as a framework does): a ref to a
/// button replaced by its equal still acts, on the new one in the same place among its equals;
/// once a new equal came before it, the ref is stale rather than a guess.
fn rerendered(driver: &WebviewDriver, origin: &str) {
    driver.navigate(&format!("{origin}/rerender.html")).unwrap();
    let page = driver.observe().unwrap();
    let adds: Vec<String> = page
        .elements
        .iter()
        .filter(|e| e.role == "button" && e.name == "Add one")
        .map(|e| e.reference.clone())
        .collect();
    assert_eq!(adds.len(), 2, "{:#?}", page.elements);
    act(
        driver,
        &page,
        Action::Click {
            reference: adds[0].clone(),
        },
    );
    // Both buttons are new nodes now: the second's old ref finds the new second.
    act(
        driver,
        &page,
        Action::Click {
            reference: adds[1].clone(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "apples:1 pears:1");
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Grow").into(),
        },
    );
    let gone = driver.act(
        &page.epoch,
        &Action::Click {
            reference: adds[1].clone(),
        },
    );
    assert!(matches!(gone, Err(ViewError::Stale(_))), "{gone:?}");
    assert_eq!(driver.observe().unwrap().title, "plums:0 apples:1 pears:1");
}

/// A page is the same one however its `performance.timeOrigin` reads later: a ref acts on it.
fn drifted(driver: &WebviewDriver, origin: &str) {
    driver.navigate(&format!("{origin}/drift.html")).unwrap();
    let page = driver.observe().unwrap();
    let count = find(&page, "button", "Count").to_string();
    for _ in 0..3 {
        act(
            driver,
            &page,
            Action::Click {
                reference: count.clone(),
            },
        );
    }
    assert_eq!(driver.observe().unwrap().title, "clicks:3");
}

/// Two tests, `driver` and `panel`, as the standard harness lists and runs tests, so a test
/// runner that asks for the list (`--list --format terse`) and then runs each by name finds them.
/// The event loop takes the main thread once per process, so the chosen ones run in one loop.
fn main() {
    // Real windows: only when asked for (a display is needed, and the screen is shared).
    if std::env::var_os("LLX_DESK_DISPLAY_TESTS").is_none() {
        return;
    }
    // The panel's and the glass's windows are shown ones: rendered, and never over what the
    // person is watching.
    // A test build's home is the one it is given (paths.rs): its own, for the pictures of
    // comments and the like.
    let home = std::env::temp_dir().join(format!("llx-desk-display-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    // SAFETY: the first thing the process does, before any thread starts.
    unsafe { std::env::set_var("DESK_WINDOWS", "unseen") };
    // SAFETY: as above.
    unsafe { std::env::set_var("LLX_DESK_TEST_DIR", &home) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--list") {
        if !args.iter().any(|arg| arg == "--ignored") {
            println!("driver: test\npanel: test");
        }
        return;
    }
    let filters: Vec<&String> = args.iter().filter(|arg| !arg.starts_with('-')).collect();
    let chosen =
        |name: &str| filters.is_empty() || filters.iter().any(|f| name.contains(f.as_str()));
    let (driving, paneling) = (chosen("driver"), chosen("panel"));
    if !driving && !paneling {
        return;
    }
    let origin = serve();
    let window = Window {
        url: format!("{origin}/form.html"),
        title: "browser driver test".into(),
        headless: true,
        viewport: browser::Viewport::DESKTOP,
    };
    // A window that cannot open ends the run with 3.
    browser::host(move |browser| {
        let run = |name: &str, test: &dyn Fn()| match catch_unwind(AssertUnwindSafe(test)) {
            Ok(()) => {
                println!("{name}: ok");
                true
            }
            Err(_) => false,
        };
        if paneling && !run("panel (first)", &|| panel::first(&browser)) {
            return 1;
        }
        #[cfg(target_os = "macos")]
        if paneling && !run("UI motion and split", &|| ui::scenario(&browser)) {
            return 1;
        }
        if paneling && !run("board (reopened)", &|| board::reopened(&browser)) {
            return 1;
        }
        if driving && !run("refs", &|| refs::scenario(&browser, &origin)) {
            return 1;
        }
        // The Event Center lies over the pages: macOS's, where the strip does.
        #[cfg(target_os = "macos")]
        if paneling && !run("event center", &|| board::center(&browser)) {
            return 1;
        }
        // Putting the window away is macOS's: elsewhere the button minimizes as the system does.
        #[cfg(target_os = "macos")]
        if paneling
            && !run("presence", &|| {
                presence::never_active(|| presence::scenario(&browser, &window, &origin));
            })
        {
            return 1;
        }
        let driver = match browser.open(window.clone()) {
            Ok(driver) => driver,
            Err(error) => {
                eprintln!("{error}");
                return 3;
            }
        };
        let driven = !driving
            || run("driver", &|| {
                driver
                    .wait_loaded(Duration::from_secs(10))
                    .expect("it loads");
                opened_twice(&browser, &driver, &window);
                second_page(&browser, &driver, &origin);
                answers_while_loading(&browser, &driver, &origin);
                starts_at_once(&browser, &driver, &origin);
                the_person_reloads(&browser, &driver);
                feedback(&browser, &driver);
                feedback_in_a_dialog(&driver);
                waits_for_the_person(&browser, &driver);
                sizes(&driver);
                told::scenario(&driver, &origin);
                scenario(&driver, &origin);
            });
        // The driver's scenario ends with its window closed: the panel's opens it again.
        let paneled = !paneling
            || run("panel", &|| {
                let reopened = driver
                    .observe()
                    .is_err()
                    .then(|| browser.open(window.clone()));
                let driver = match reopened {
                    Some(opened) => opened.expect("it opens again"),
                    None => driver.clone(),
                };
                panel::scenario(&browser, &driver);
                panel::glass(&browser, &driver, &window, &origin);
                board::scenario(&browser);
                // Last: the window in use is the new one after it.
                panel::windows(&browser);
            });
        if driven && paneled { 0 } else { 1 }
    });
}
