//! A board as a panel of the one window: its page runs its script by its hash, shows the board
//! painted into it, and what the person does in it comes back as their actions.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use llxterm::board::{Board, Worker};
use llxterm::browser::{BOARD, Browser, Pane};
use serde_json::json;

fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    let title = "desk · board · shop";
    let panel = browser.panel(BOARD + 1, title, llxterm::board::page(title));
    let posted: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = Arc::clone(&posted);
    panel.keyboard(Arc::new(move |text: &str| {
        kept.lock().unwrap().push(text.to_string())
    }));
    let loaded: Arc<Mutex<bool>> = Arc::default();
    let seen = Arc::clone(&loaded);
    panel.sizing(Arc::new(move |pane, _, _| {
        if pane == Pane::Loaded {
            *seen.lock().unwrap() = true;
        }
    }));
    // Its script ran (the policy let it, by its hash) and it said it is ready.
    until("the board's script never ran", || {
        browser.peek(&panel, "typeof B") == Ok(json!("function"))
    });
    until("the board never said it loaded", || *loaded.lock().unwrap());

    let mut board = Board::default();
    for title in ["Header", "Cart on mobile", "<b>not markup</b>"] {
        board.add(title, "", None, "agent").unwrap();
    }
    board.take(Some("c2"), Worker(Some("tests"))).unwrap();
    board.review("c2", Worker(Some("tests")), "fixed").unwrap();
    panel.paint(&llxterm::board::frame(
        &board,
        "b1",
        "shop",
        llxterm::board::now(),
        &json!([{"id": "codex", "name": "Codex"}]),
    ));
    // A card waiting in Todo offers to be run by an agent.
    let read = "[document.querySelectorAll('.card').length, \
        document.querySelector('.lane[data-col=review] .card .t').textContent, \
        document.querySelector('.card[data-id=c3] .t').textContent, \
        document.querySelectorAll('.card .t b').length, \
        document.querySelector('.card[data-id=c1] .quick').title]";
    until("the board was never painted", || {
        browser.peek(&panel, read)
            == Ok(json!([
                3,
                "Cart on mobile",
                "<b>not markup</b>",
                0,
                "Run with an agent…"
            ]))
    });

    // The window lists it as the board it is.
    let listed = browser.listing().unwrap();
    assert!(
        listed
            .iter()
            .any(|tab| tab.id == "b1" && tab.kind == "board"),
        "{listed:?}"
    );

    // The person accepts the card under review: the action comes back whole (the window takes
    // off the page's `k`).
    browser
        .peek(
            &panel,
            "document.querySelector('.lane[data-col=review] .act.ok').click(); 1",
        )
        .unwrap();
    until("the person's action never came back", || {
        posted
            .lock()
            .unwrap()
            .iter()
            .any(|text| text == r#"{"op":"accept","card":"c2"}"#)
    });
    // A refusal is said in the page.
    panel.paint(&llxterm::board::refused("there is no card c9 on the board"));
    until("the refusal was never shown", || {
        browser.peek(&panel, "document.getElementById('toast').textContent")
            == Ok(json!("there is no card c9 on the board"))
    });
    drop(panel);
}

/// The person closes the board's tab, the window's last: the window stays with no tab, and the
/// agent's `show` gives the board a tab again, painted as it is.
pub(crate) fn reopened(browser: &Browser) {
    let home = std::env::temp_dir().join(format!("llxterm-board-{}", std::process::id()));
    std::fs::create_dir_all(home.join(".git")).unwrap();
    let shots = home.join("shots");
    let desk = llxterm::desk::Desk::new(
        Some(browser.clone()),
        true,
        shots,
        Arc::new(llxterm::desk::Quiet),
        Arc::new(llxterm::store::Store::memory().unwrap()),
    );
    let project = desk.project(&home).unwrap();
    let owner = llxterm::projects::owner(&project, None);
    let call = llxterm::desk::Call {
        owner: &owner,
        project: &project,
        agent: None,
        cwd: &home,
        vars: None,
        peek: false,
        again: None,
        person: false,
        window: None,
        from: None,
        runs: None,
    };
    let opened = desk
        .window(&call, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    assert!(opened.starts_with("board: b1"), "{opened}");
    desk.window(
        &call,
        &json!({"action": "interact", "tab": "b1", "op": "add", "title": "Cart"}),
    )
    .unwrap();
    let listed =
        |id: &str| (browser.listing()).is_some_and(|tabs| tabs.iter().any(|tab| tab.id == id));
    until("the board's tab is shown", || listed("b1"));
    browser.click("close b1");
    until("the window stays with no tab", || {
        browser.listing().is_some_and(|tabs| tabs.is_empty())
    });
    let shown = desk
        .window(&call, &json!({"action": "show", "tab": "b1"}))
        .unwrap();
    assert!(shown.contains("c1 Cart"), "{shown}");
    until("the board has a tab again", || listed("b1"));
    browser.click("close b1");
    until("and goes again", || !listed("b1"));
    let _ = std::fs::remove_dir_all(&home);
}

/// A card handed in is news in the Event Center; the person opens it (its shortcut: the bar has no
/// button for it), sees it, and accepts the card from there, which moves it to Done on the board.
#[cfg(target_os = "macos")]
pub(crate) fn center(browser: &Browser) {
    let home = std::env::temp_dir().join(format!("llxterm-center-{}", std::process::id()));
    std::fs::create_dir_all(home.join(".git")).unwrap();
    let desk = llxterm::desk::Desk::new(
        Some(browser.clone()),
        true,
        home.join("shots"),
        Arc::new(llxterm::desk::Quiet),
        Arc::new(llxterm::store::Store::memory().unwrap()),
    );
    let project = desk.project(&home).unwrap();
    let owner = llxterm::projects::owner(&project, None);
    let call = llxterm::desk::Call {
        owner: &owner,
        project: &project,
        agent: None,
        cwd: &home,
        vars: None,
        peek: false,
        again: None,
        person: false,
        window: None,
        from: None,
        runs: None,
    };
    let act = |args: serde_json::Value| desk.window(&call, &args).unwrap();
    act(json!({"action": "open", "kind": "board"}));
    act(json!({"action": "interact", "tab": "b1", "op": "add", "title": "Cart"}));
    act(json!({"action": "interact", "tab": "b1", "op": "take", "card": "c1"}));
    act(
        json!({"action": "interact", "tab": "b1", "op": "review", "card": "c1", "note": "fixed it"}),
    );
    // The bar has a button for the center, with a dot while something new waits for the person.
    until("the center's button shows that something waits", || {
        browser.peek_strip("document.querySelector('#ev .d')?.hidden") == Ok(json!(false))
    });
    browser.click("events");
    until("the center lists it, new until it is seen", || {
        let read = "[document.querySelectorAll('#menu .ev').length, document.querySelector('#menu .ev .n')?.textContent]";
        browser.peek_strip(read) == Ok(json!([1, "Cart"]))
    });
    let accepted = browser.peek_strip("document.querySelector('#menu .ev .act').click(); 1");
    assert_eq!(accepted, Ok(json!(1)));
    until("the card is accepted from the center", || {
        act(json!({"action": "interact", "tab": "b1"})).contains("Done (1")
    });
    until("and the center is empty", || {
        browser.peek_strip("document.querySelectorAll('#menu .ev').length + ':' + getComputedStyle(document.getElementById('menu')).display") == Ok(json!("0:none"))
    });
    assert_eq!(
        browser.peek_strip("document.querySelector('#ev .d').hidden"),
        Ok(json!(true)),
        "nothing waits: the button is quiet"
    );
    act(json!({"action": "close", "tab": "b1"}));
    desk.shutdown();
    let _ = std::fs::remove_dir_all(&home);
}
