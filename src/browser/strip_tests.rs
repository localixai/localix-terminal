//! Tests of `strip`.

#[path = "strip/docs_tests.rs"]
mod documents;
#[path = "strip/input_tests.rs"]
mod input;
#[path = "strip/page_tests.rs"]
mod policy;

use super::*;
use crate::browser::Visited;

#[test]
fn the_tabs_reach_the_page_as_data() {
    let items = [
        Item {
            key: Key::Web(1),
            title: "</script><img src=x onerror=alert(1)>".into(),
            detail: "https://a.example/\"><b>".into(),
            group: "proj".into(),
            agent: None,
            waits: None,
            of: None,
        },
        Item {
            key: Key::Term(2),
            title: "session 2 · python3".into(),
            detail: String::new(),
            group: "proj".into(),
            agent: None,
            waits: None,
            of: None,
        },
    ];
    let script = script(&items, Key::Term(2), None, &none());
    assert!(
        script.starts_with("S(") && script.ends_with(')'),
        "{script}"
    );
    let json = script
        .strip_prefix("S(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(data["shown"], "t2");
    assert_eq!(data["tabs"][0]["key"], "w1");
    assert_eq!(data["tabs"][0]["kind"], "web");
    assert_eq!(
        data["tabs"][0]["title"],
        "</script><img src=x onerror=alert(1)>"
    );
    assert_eq!(data["tabs"][1]["kind"], "term");
    // What the address field shows: each tab's page. The pages seen are the toolbar's alone.
    assert_eq!(data["tabs"][0]["detail"], "https://a.example/\"><b>");
    assert!(data.get("visited").is_none(), "{data}");
    // The page sets titles as text, never as markup.
    assert!(PAGE.contains("textContent") && !PAGE.contains("innerHTML"));
}

#[test]
fn the_pages_seen_reach_the_toolbar_as_data_of_their_own() {
    let (title, url) = ("<i>Shop</i>".to_string(), "https://shop.test/a".to_string());
    let script = visited(&[Visited { title, url }]);
    let json = (script.strip_prefix("VI(")).and_then(|s| s.strip_suffix(')'));
    let data: serde_json::Value = serde_json::from_str(json.unwrap()).unwrap();
    assert_eq!(data[0]["title"], "<i>Shop</i>");
    assert_eq!(data[0]["url"], "https://shop.test/a");
    assert!(PAGE.contains("function VI(visited)"));
}

#[test]
fn an_empty_title_reads_as_what_the_tab_is() {
    let items = [Item {
        key: Key::Web(1),
        title: String::new(),
        detail: String::new(),
        group: String::new(),
        agent: None,
        waits: None,
        of: None,
    }];
    let script = script(&items, Key::Web(1), None, &none());
    assert!(script.contains("\"title\":\"New tab\""), "{script}");
}

fn item(key: Key, title: &str, group: &str) -> Item {
    Item {
        key,
        title: title.into(),
        detail: String::new(),
        group: group.into(),
        agent: None,
        waits: None,
        of: None,
    }
}

#[test]
fn one_project_draws_as_it_always_has() {
    let items = [
        item(Key::Web(1), "Docs", "proj"),
        item(Key::Term(2), "make", "proj"),
    ];
    assert_eq!(
        script(&items, Key::Web(1), None, &none()),
        r#"S({"places":[],"project":"","shown":"w1","side":null,"tabs":[{"detail":"","group":"proj","key":"w1","kind":"web","title":"Docs"},{"detail":"","group":"proj","key":"t2","kind":"term","title":"make"}]})"#
    );
}

#[test]
fn each_tab_says_its_project_and_the_switcher_names_the_one_shown() {
    let items = [
        item(Key::Web(1), "Docs", "1"),
        item(Key::Term(2), "make", "2"),
        item(Key::Web(3), "Issues", "1"),
    ];
    let (shop, blog) = (
        crate::browser::Place {
            key: "1".into(),
            name: "<b>shop</b>".into(),
            folder: "/work/shop".into(),
        },
        crate::browser::Place {
            key: "2".into(),
            name: "blog".into(),
            folder: "/work/blog".into(),
        },
    );
    let projects = Projects {
        shown: "1",
        places: vec![(&shop, false), (&blog, true)],
    };
    let script = script(&items, Key::Web(1), None, &projects);
    let json = script
        .strip_prefix("S(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    let groups: Vec<&str> = (data["tabs"].as_array().unwrap().iter())
        .map(|tab| tab["group"].as_str().unwrap())
        .collect();
    assert_eq!(groups, ["1", "2", "1"]);
    assert_eq!(data["project"], "1");
    assert_eq!(
        data["places"],
        serde_json::json!([
            {"key": "1", "name": "<b>shop</b>", "marked": false, "folder": "/work/shop"},
            {"key": "2", "name": "blog", "marked": true, "folder": "/work/blog"},
        ])
    );
    // A crumb's name is set as text.
    assert!(PAGE.contains("name.textContent = parts.name;"));
}

#[test]
fn the_history_menu_reaches_the_page_as_data() {
    let row = |at, key: Option<Key>, kind, state| Row {
        at,
        key,
        kind,
        title: "\"</script><b>x</b>".into(),
        detail: "https://a.example/".into(),
        state,
        group: "proj".into(),
    };
    let rows = [
        row(7, Some(Key::Term(2)), "term", State::Hidden),
        row(3, None, "web", State::Closed),
    ];
    let open = menu(Some(&rows));
    let json = open
        .strip_prefix("M(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(data[0]["at"], 7);
    assert_eq!(data[0]["key"], "t2");
    assert_eq!(data[0]["kind"], "term");
    assert_eq!(data[0]["state"], "hidden");
    assert_eq!(data[0]["title"], "\"</script><b>x</b>");
    assert_eq!(data[1]["kind"], "web");
    assert_eq!(data[1]["key"], serde_json::Value::Null);
    assert_eq!(data[1]["state"], "closed");
    assert_eq!(data[1]["detail"], "https://a.example/");
    assert_eq!(menu(None), "M(null)");
}

#[test]
fn the_bar_holds_the_projects_and_the_crumbs_and_the_page_its_own_toolbar() {
    let at = |id: &str| PAGE.find(&format!("id=\"{id}\"")).unwrap();
    // The bar (beside the window's buttons): the crumbs, then the window's buttons; nothing
    // else. The browser's toolbar after it: back, forward, reload, the address, the page's
    // tools. The tabs' row under them.
    assert!(at("bar") < at("cr") && at("cr") < at("end") && at("end") < at("browse"));
    assert!(!PAGE.contains("id=\"logo\"") && !PAGE.contains("id=\"pt\""));
    assert!(at("browse") < at("bk") && at("bk") < at("addr") && at("addr") < at("fs"));
    assert!(at("fs") < at("tabs"));
    assert!(PAGE.contains("html[data-glass] #bar{padding-left:78px}"));
}

#[test]
fn the_strip_keeps_the_windows_own_colours_over_every_tab() {
    // Its text takes the shared ink of every app page (frame.css), never a tab's colours.
    assert!(!PAGE.contains("data-tint") && !PAGE.contains("--ink:"));
}

#[test]
fn the_strip_shows_whose_turn_it_is() {
    use super::{Turn, turn};
    assert_eq!(Turn::of(false, false, 0), Turn::Idle);
    assert_eq!(Turn::of(true, false, 0), Turn::Acting);
    // An action under way when the person turned comments on still shows until it settles.
    assert_eq!(Turn::of(true, true, 0), Turn::Acting);
    assert_eq!(Turn::of(false, true, 0), Turn::Paused);
    assert_eq!(Turn::of(false, true, 2), Turn::Waiting);
    assert_eq!(turn(Turn::Idle), r#"A("")"#);
    assert_eq!(turn(Turn::Waiting), r#"A("waiting")"#);
}

#[test]
fn a_board_is_a_panel_past_the_terminals_named_b() {
    let board = Key::parse("b3").unwrap();
    assert_eq!(board, Key::Term(BOARD + 3));
    assert_eq!((board.name().as_str(), board.kind()), ("b3", "board"));
    assert_eq!(Key::Term(2).kind(), "term");
    assert_eq!(Key::parse("b0"), None);
    assert_eq!(Key::parse(&format!("t{}", BOARD + 1)), None);
    let items = [item(Key::Term(BOARD + 1), "", "proj")];
    assert!(
        script(&items, Key::Term(BOARD + 1), None, &none())
            .contains(r#""kind":"board","title":"Board""#)
    );
}

#[test]
fn a_terminal_running_an_agent_carries_its_mark_and_what_it_does() {
    let items = [Item {
        key: Key::Term(3),
        title: "session 3 · claude --resume".into(),
        detail: "Fix the cart button".into(),
        group: "proj".into(),
        agent: None,
        waits: None,
        of: None,
    }];
    let script = script(&items, Key::Term(3), Some(true), &none());
    let data: serde_json::Value = serde_json::from_str(
        script
            .strip_prefix("S(")
            .unwrap()
            .strip_suffix(')')
            .unwrap(),
    )
    .unwrap();
    let tab = &data["tabs"][0];
    assert_eq!(tab["agent"], "claude");
    assert_eq!(tab["name"], "Claude Code");
    assert_eq!(tab["detail"], "Fix the cart button");
}

#[test]
fn a_shell_running_an_agent_in_front_carries_its_mark() {
    // The person typed `codex` in the shell: the program in front says so, not the title.
    let items = [Item {
        key: Key::Term(4),
        title: "session 4 · shell".into(),
        detail: "okak@Mac:~".into(),
        group: "proj".into(),
        agent: Some("codex"),
        waits: None,
        of: None,
    }];
    let drawn = script(&items, Key::Term(4), Some(true), &none());
    assert!(drawn.contains(r#""agent":"codex""#), "{drawn}");
    assert!(drawn.contains(r#""name":"Codex""#), "{drawn}");
}

#[test]
fn a_tab_that_waits_for_the_person_carries_the_words_of_what_waits() {
    let listed = |waits: Option<&str>| {
        let items = [Item {
            key: Key::Term(4),
            title: "session 4 · shell".into(),
            detail: "okak@Mac:~".into(),
            group: "proj".into(),
            agent: Some("claude"),
            waits: waits.map(str::to_string),
            of: None,
        }];
        script(&items, Key::Web(1), Some(true), &none())
    };
    let drawn = listed(Some("Claude asks: Which database?"));
    assert!(
        drawn.contains(r#""waits":"Claude asks: Which database?""#),
        "{drawn}"
    );
    // A tab nothing waits in says nothing of it.
    let drawn = listed(None);
    assert!(!drawn.contains("waits"), "{drawn}");
}

#[test]
fn no_two_functions_of_the_strip_share_a_name() {
    // A second `function mark` is hoisted over the first, and the side drew "undefined".
    let mut names: Vec<&str> = (PAGE.split("function ").skip(1))
        .filter_map(|rest| rest.split('(').next())
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "a function name is declared twice");
}

/// No project known: the window's first moments.
fn none() -> Projects<'static> {
    Projects {
        shown: "",
        places: vec![],
    }
}

#[test]
fn the_page_is_told_the_agents_installed_as_data() {
    let told = starts(r#"[{"at":0,"id":"claude","name":"Claude Code"}]"#);
    assert_eq!(told, r#"AG([{"at":0,"id":"claude","name":"Claude Code"}])"#);
    // What is not a list of agents is none of them, never script.
    assert_eq!(starts("alert(1)"), "AG([])");
}
