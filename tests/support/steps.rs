//! Several steps in one call on a real page, as an agent's one command sends them: open a page it
//! has not read, click a control by its name, wait for what the click shows. The bytes the agent
//! reads for that flow are measured against the two calls it took before.

use std::sync::Arc;

use llxterm::browser::Browser;
use serde_json::{Value, json};

/// The JSON a command line sends, from the words after `llxterm`.
fn sent(argv: &[&str]) -> Value {
    let argv: Vec<String> = argv.iter().map(|word| (*word).to_string()).collect();
    let steps = llxterm::short::steps(&argv, &mut || Err("no stdin".into())).unwrap();
    json!({"action": "steps", "steps": steps})
}

/// The ref the outline gives the control `line` names.
fn ref_of(outline: &str, line: &str) -> String {
    let found = (outline.lines())
        .find(|at| at.contains(line))
        .unwrap_or_else(|| panic!("no `{line}` in\n{outline}"));
    (found.rsplit(" [").next())
        .and_then(|last| last.split(']').next())
        .unwrap_or_else(|| panic!("no ref in `{found}`"))
        .to_string()
}

pub(crate) fn scenario(browser: &Browser, origin: &str) {
    let home = std::env::temp_dir().join(format!("llxterm-steps-{}", std::process::id()));
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
    let url = format!("{origin}/save.html");

    // Before: the page is opened and read, then its button is clicked by the ref read there.
    let opening = json!({"action": "open", "url": url}).to_string();
    let opened = desk
        .window(&call, &serde_json::from_str(&opening).unwrap())
        .unwrap();
    let tab = opened.split_whitespace().next().unwrap().to_string();
    let save = ref_of(&opened, "button \"Save\"");
    let clicking =
        json!({"action": "interact", "tab": tab, "click": save, "wait_for": "Saved"}).to_string();
    let clicked = desk
        .window(&call, &serde_json::from_str(&clicking).unwrap())
        .unwrap();
    assert!(clicked.contains("\"Saved\" appeared"), "{clicked}");
    let before = [
        format!("llxterm window '{opening}'"),
        format!("llxterm window '{clicking}'"),
        opened,
        clicked,
    ];
    desk.window(&call, &json!({"action": "close", "tab": tab}))
        .unwrap();

    // After: one command, on a page not read yet.
    let argv = ["open", &url, "+", "click", "Save", "+", "wait", "Saved"];
    let answered = desk.call(&call, &sent(&argv)).unwrap();
    let text = answered.text.clone();
    assert!(!answered.missed, "{text}");
    assert!(
        text.starts_with("1. w")
            && text.contains("\n2. w")
            && text.contains("Clicked \"Save\" (e")
            && text.contains("\n3. w")
            && text.contains("\"Saved\" appeared"),
        "{text}"
    );
    // The agent never read the page: it is read whole after the last step, once.
    assert_eq!(
        text.matches("### Page (untrusted content)").count(),
        1,
        "{text}"
    );
    let after = [format!("llxterm {}", argv.join(" ")), text];
    let bytes = |parts: &[String]| parts.iter().map(String::len).sum::<usize>();
    println!(
        "steps: open a page, click, check: {} bytes in two calls before, {} in one after",
        bytes(&before),
        bytes(&after)
    );

    // What was read whole is what the next call is told against: a click that changes
    // nothing says so, not the page again.
    let again = desk.call(&call, &sent(&["click", "Cancel"])).unwrap();
    assert!(
        !again.text.contains("### Page (untrusted content)"),
        "{}",
        again.text
    );

    // A ref the page does not have, or a name no control has, misses and stops.
    let gone = desk
        .call(&call, &sent(&["click", "e9999", "+", "wait", "Saved"]))
        .unwrap();
    assert!(
        gone.missed && gone.text.contains("Stopped at step 1 of 2"),
        "{}",
        gone.text
    );
    let nameless = desk.call(&call, &sent(&["click", "Sav"])).unwrap();
    assert!(
        nameless.missed && nameless.text.contains("\"Save draft\""),
        "{}",
        nameless.text
    );
    missed_control_stops_its_call(&desk, &call, &answered.text);
    desk.end(&owner);
    let _ = std::fs::remove_dir_all(&home);
}

/// One `interact` whose field is not found stops there: its keys are not pressed (a fill that
/// missed never sends the form with Enter), nor its wait waited for, and it says the closest
/// names. `opened` is the answer whose first step opened the page.
fn missed_control_stops_its_call(
    desk: &llxterm::desk::Desk,
    call: &llxterm::desk::Call,
    opened: &str,
) {
    let tab = (opened.strip_prefix("1. "))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no tab in {opened}"));
    let status = || {
        let read = json!({"action": "interact", "tab": tab,
            "script": "document.getElementById('status').textContent"});
        desk.call(call, &read).unwrap().text
    };
    let enter = |field: &str| {
        let act = json!({"action": "interact", "tab": tab, "fill": {field: "Grace"},
            "keys": ["Enter"], "wait_for": "Sent"});
        desk.call(call, &act).unwrap()
    };
    // Found: the field is filled and Enter sends the form.
    let found = enter("Name");
    assert!(
        !found.missed && found.text.contains("\"Sent\" appeared"),
        "{}",
        found.text
    );
    let clear = json!({"action": "interact", "tab": tab,
        "script": "document.getElementById('status').textContent = ''"});
    desk.call(call, &clear).unwrap();
    // Not found: nothing after it is done.
    let missed = enter("Nam");
    assert!(missed.missed, "{}", missed.text);
    assert!(
        missed
            .text
            .contains("no control is named \"Nam\"; the closest: \"Name\" [e"),
        "{}",
        missed.text
    );
    assert!(
        !missed.text.contains("Pressed") && !missed.text.contains("Sent"),
        "{}",
        missed.text
    );
    let now = status();
    assert!(
        !now.contains("Sent"),
        "Enter sent the form after the miss: {now}"
    );
}
