//! More terminals than the app once held (16), in one window: each opens and runs, the side lists
//! every one, and a terminal whose tab the person closed, its program running on out of sight,
//! is never ended to make room for another.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::Browser;
use serde_json::{Value, json};

/// Terminals opened: more than the app held at once before it read its room from the machine.
const OPEN: usize = 20;

fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !want() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    let home = std::env::temp_dir().join(format!("llxterm-room-{}", std::process::id()));
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
    let act = |args: Value| desk.window(&call, &args).unwrap();
    let group = llxterm::projects::key(project.id);
    browser.go(&group);
    let open = |n: usize| {
        let command = format!("printf 'ready %s' {n}; exec sleep 120");
        let said = act(
            json!({"action": "open", "kind": "terminal", "command": command,
            "wait_for": format!("ready {n}")}),
        );
        assert!(!said.starts_with("not opened"), "terminal {n}: {said}");
    };
    for n in 1..=OPEN {
        open(n);
    }
    let terminals = || {
        (browser.listing().unwrap_or_default().iter())
            .filter(|tab| tab.group == group && tab.id.starts_with('t'))
            .count()
    };
    until("the side lists every terminal", || terminals() == OPEN);
    let counted = |n: usize| desk.status().contains(&format!("terminals: {n}\n"));
    assert!(counted(OPEN), "{}", desk.status());

    // The person closes the first one's tab: its program runs on, and the next terminal opens
    // beside it rather than in its place.
    browser.click("close t1");
    until("the closed tab leaves the side", || terminals() == OPEN - 1);
    open(OPEN + 1);
    assert!(counted(OPEN + 1), "{}", desk.status());
    let first = act(json!({"action": "interact", "tab": "t1"}));
    assert!(!first.contains("exited"), "its program still runs: {first}");

    let all: Vec<String> = (1..=OPEN + 1).map(|n| format!("t{n}")).collect();
    act(json!({"action": "close", "tabs": all}));
    desk.shutdown();
    let _ = std::fs::remove_dir_all(&home);
}
