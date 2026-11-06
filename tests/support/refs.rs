//! A page worked through the desk by the refs of its outline, as an agent works it: `fill` a field,
//! `select` an option, `click` a button, each by the ref the snapshot gave it.

use std::sync::Arc;

use llx_desk::browser::Browser;
use serde_json::json;

/// The ref the outline gives the control `line` names: `textbox "City" [e4]` → `e4`.
fn ref_of(outline: &str, line: &str) -> String {
    let found = (outline.lines())
        .find(|at| at.contains(line))
        .unwrap_or_else(|| panic!("no `{line}` in\n{outline}"));
    (found.rsplit(" [").next())
        .and_then(|last| last.split(']').next())
        .filter(|at| at.starts_with('e'))
        .unwrap_or_else(|| panic!("no ref in `{found}`"))
        .to_string()
}

pub(crate) fn scenario(browser: &Browser, origin: &str) {
    let home = std::env::temp_dir().join(format!("llx-desk-refs-{}", std::process::id()));
    std::fs::create_dir_all(home.join(".git")).unwrap();
    let desk = llx_desk::desk::Desk::new(
        Some(browser.clone()),
        true,
        home.join("shots"),
        Arc::new(llx_desk::desk::Quiet),
        Arc::new(llx_desk::store::Store::memory().unwrap()),
    );
    let project = desk.project(&home).unwrap();
    let owner = llx_desk::projects::owner(&project, None);
    let call = llx_desk::desk::Call {
        owner: &owner,
        project: &project,
        agent: None,
        cwd: &home,
        vars: None,
        peek: false,
    };
    let url = format!("{origin}/form.html");
    let opened = desk
        .window(&call, &json!({"action": "open", "url": url}))
        .unwrap();
    let tab = opened.split_whitespace().next().unwrap().to_string();
    let accept = ref_of(&opened, "button \"Accept\"");
    let closed = desk
        .window(
            &call,
            &json!({"action": "interact", "tab": tab, "click": accept}),
        )
        .unwrap();
    assert!(closed.contains(&format!("Clicked {accept}.")), "{closed}");

    let (city, class, search) = (
        ref_of(&opened, "textbox \"City\""),
        ref_of(&opened, "combobox \"Class\""),
        ref_of(&opened, "button \"Search\""),
    );
    let sent = desk
        .window(
            &call,
            &json!({"action": "interact", "tab": tab, "fill": {city.clone(): "Paris"},
                "select": {format!("[{class}]"): "Business"}, "click": [format!("@{search}")],
                "wait_for": "sent:Paris:Business"}),
        )
        .unwrap();
    assert!(
        sent.contains(&format!("Filled {city}."))
            && sent.contains(&format!("Selected \"Business\" in {class}."))
            && sent.contains(&format!("Clicked {search}.")),
        "{sent}"
    );
    let title = desk
        .window(
            &call,
            &json!({"action": "interact", "tab": tab, "script": "document.title"}),
        )
        .unwrap();
    assert!(title.contains("sent:Paris:Business"), "{title}");

    // A ref the page does not have stops there, and says so; one that is no ref is refused.
    let gone = desk
        .window(
            &call,
            &json!({"action": "interact", "tab": tab, "click": "e9999"}),
        )
        .unwrap();
    assert!(gone.contains("Clicked e9999 not done"), "{gone}");
    let bad = desk
        .window(
            &call,
            &json!({"action": "interact", "tab": tab, "click": "Search"}),
        )
        .unwrap_err();
    assert!(
        bad.message().contains("is not a control's ref"),
        "{}",
        bad.message()
    );
    // A screenshot is a file, named in the answer.
    let shot = desk
        .window(&call, &json!({"action": "screenshot", "tab": tab}))
        .unwrap();
    let path = (shot.lines())
        .find_map(|line| line.strip_prefix("Screenshot saved ("))
        .and_then(|rest| rest.split("): ").nth(1))
        .unwrap_or_else(|| panic!("no file named in\n{shot}"));
    assert!(std::path::Path::new(path).is_file(), "{path}");
    desk.end(&owner);
    let _ = std::fs::remove_dir_all(&home);
}
