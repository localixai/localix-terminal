//! What a terminal's program says of its state (OSC 7501), as the person sees it in the window:
//! at work, a still ring on its tab's icon in the side; waiting for them, out of sight, the
//! Event Center's dot there with its words under the name, and a line in the Event Center that
//! shows the tab, titled by the terminal; failed, the same dot and the line, replaced in place,
//! among the programs that ended.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use llxterm::browser::Browser;
use serde_json::{Value, json};

fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The side's row once `want` holds of it, within a while: what it showed last, if never.
fn shows(row: &dyn Fn() -> Value, what: &str, want: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let now = row();
        if want(&now) {
            return now;
        }
        assert!(Instant::now() < deadline, "{what}: {now}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A report as `printf` writes it.
fn report(body: &str) -> String {
    format!("\\033]7501;{body}\\033\\\\")
}

/// What the side shows of terminal t1's row: its marks, its tooltip and its second line.
const ROW: &str = "(() => { const row = document.querySelector('#sl [data-key=\"t1\"]'); \
    if (!row) return { none: [...document.querySelectorAll('[data-key]')].map((e) => e.className + ':' + e.dataset.key), part: document.documentElement.dataset.part }; \
    const ring = row.querySelector('.ic > .ring'); \
    return { ring: ring ? ring.querySelectorAll('svg > *').length : 0, \
      turns: ring ? getComputedStyle(ring).animationName + getComputedStyle(ring.firstChild).animationName : '', \
      dot: !!row.querySelector('.ic > .d'), title: row.title, under: row.querySelector('.u').textContent } })()";

pub(crate) fn scenario(browser: &Browser) {
    let home = std::env::temp_dir().join(format!("llxterm-status-{}", std::process::id()));
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
    let asks = base64::engine::general_purpose::STANDARD.encode("approve Bash: npm test");
    // The program reports, then waits for a line before it reports again, as an agent's turn.
    let command = format!(
        "printf '{}one'; read a; printf '{}two'; read b; printf '{}three'; sleep 30",
        report("state=working:app=claude-code:progress=40"),
        report(&format!(
            "state=blocked:app=claude-code:kind=permission:msg={asks}"
        )),
        report("state=error:app=claude-code"),
    );
    act(json!({"action": "open", "kind": "terminal", "command": command, "wait_for": "one"}));
    // The window shows the project, as the person going to it does.
    browser.go(&llxterm::projects::key(project.id));
    let row = || browser.peek_tabs(ROW).unwrap_or(Value::Null);
    let at_work = shows(
        &row,
        "the tab at work shows a still ring with its share",
        |row| row["ring"] == json!(2) && row["dot"] == json!(false),
    );
    assert_eq!(at_work["turns"], json!("nonenone"), "nothing of it moves");
    assert!(
        at_work["title"]
            .as_str()
            .unwrap()
            .contains("— working, 40%"),
        "{at_work}"
    );

    // Another tab in sight: what the program waits for is the Event Center's, and its dot.
    act(json!({"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}));
    act(json!({"action": "interact", "tab": "t1", "input": "\n", "wait_for": "two"}));
    shows(
        &row,
        "the tab out of sight that waits has the dot and its words",
        |row| {
            row["dot"] == json!(true)
                && row["ring"] == json!(0)
                && row["under"] == json!("approve Bash: npm test")
        },
    );
    assert!(row()["title"].as_str().unwrap().contains("— waits for you"));
    browser.click("events");
    // Whether each line is titled by its terminal (its command), never by the program's words,
    // and the first of its words: its detail, the app the program names then what it says.
    let lines = "[...document.querySelectorAll('#menu .ev')].map((ev) => \
        [ev.querySelector('.n').textContent.startsWith('printf '), \
         ev.querySelector('.u').textContent.split(' · ').slice(0, 2).join(' · ')])";
    until(
        "the Event Center lists what it waits for, titled by its terminal, its app in the words",
        || browser.peek_strip(lines) == Ok(json!([[true, "Claude Code · approve Bash: npm test"]])),
    );
    browser.click("events");

    act(json!({"action": "interact", "tab": "t1", "input": "\n", "wait_for": "three"}));
    browser.click("events");
    // The program's own line is replaced in place: no one typed in the terminal between.
    until("a failure is a line among the programs that ended", || {
        browser.peek_strip(lines) == Ok(json!([[true, "Claude Code · Stopped on an error"]]))
    });
    browser.click("events");
    assert_eq!(row()["dot"], json!(true), "it wants the person too");

    act(json!({"action": "close", "tab": "t2"}));
    act(json!({"action": "close", "tab": "t1"}));
    desk.shutdown();
    let _ = std::fs::remove_dir_all(&home);
}
