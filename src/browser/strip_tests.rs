//! Tests of `strip`.

use super::*;
use crate::browser::Visited;

#[test]
fn a_click_names_a_tab_and_what_to_do_with_it() {
    assert_eq!(click("show w3"), Some(Click::Show(Key::Web(3))));
    assert_eq!(click("show t1"), Some(Click::Show(Key::Term(1))));
    assert_eq!(click("close t2"), Some(Click::Close(Key::Term(2))));
    assert_eq!(click("close w1"), Some(Click::Close(Key::Web(1))));
}

#[test]
fn anything_else_from_the_strip_is_ignored() {
    for junk in [
        "",
        "show",
        "show x3",
        "show w",
        "show w-1",
        "open w1",
        "show w1 t2",
        "show w99999999999",
    ] {
        assert_eq!(click(junk), None, "{junk:?}");
    }
}

#[test]
fn the_tabs_reach_the_page_as_data() {
    let items = [
        Item {
            key: Key::Web(1),
            title: "</script><img src=x onerror=alert(1)>".into(),
            detail: "https://a.example/\"><b>".into(),
            group: "proj".into(),
            agent: None,
        },
        Item {
            key: Key::Term(2),
            title: "session 2 · python3".into(),
            detail: String::new(),
            group: "proj".into(),
            agent: None,
        },
    ];
    let visited = [Visited {
        title: "<i>Habr</i>".into(),
        url: "https://habr.com/ru/post/210288/".into(),
    }];
    let script = script(&items, Key::Term(2), &visited, None, &none());
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
    // What the address field shows and suggests: each tab's page, the pages seen.
    assert_eq!(data["tabs"][0]["detail"], "https://a.example/\"><b>");
    assert_eq!(data["visited"][0]["title"], "<i>Habr</i>");
    assert_eq!(
        data["visited"][0]["url"],
        "https://habr.com/ru/post/210288/"
    );
    // The page sets titles as text, never as markup.
    assert!(PAGE.contains("textContent") && !PAGE.contains("innerHTML"));
}

#[test]
fn an_empty_title_reads_as_what_the_tab_is() {
    let items = [Item {
        key: Key::Web(1),
        title: String::new(),
        detail: String::new(),
        group: String::new(),
        agent: None,
    }];
    let script = script(&items, Key::Web(1), &[], None, &none());
    assert!(script.contains("\"title\":\"New tab\""), "{script}");
}

fn item(key: Key, title: &str, group: &str) -> Item {
    Item {
        key,
        title: title.into(),
        detail: String::new(),
        group: group.into(),
        agent: None,
    }
}

#[test]
fn one_project_draws_as_it_always_has() {
    let items = [
        item(Key::Web(1), "Docs", "proj"),
        item(Key::Term(2), "make", "proj"),
    ];
    assert_eq!(
        script(&items, Key::Web(1), &[], None, &none()),
        r#"S({"places":[],"project":"","shown":"w1","side":null,"tabs":[{"detail":"","group":"proj","key":"w1","kind":"web","title":"Docs"},{"detail":"","group":"proj","key":"t2","kind":"term","title":"make"}],"visited":[]})"#
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
    let script = script(&items, Key::Web(1), &[], None, &projects);
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
fn the_toolbar_and_the_history_button_say_what_they_are() {
    assert_eq!(click("back"), Some(Click::Back));
    assert_eq!(click("forward"), Some(Click::Forward));
    assert_eq!(click("history"), Some(Click::History));
    assert_eq!(click("visit 2"), Some(Click::Visit(2)));
    assert_eq!(click("comment"), Some(Click::Comment));
    assert_eq!(click("drag"), Some(Click::Drag));
    assert_eq!(click("zoom"), Some(Click::Zoom));
    assert_eq!(click("collapse"), Some(Click::Collapse));
    assert_eq!(click("quit"), Some(Click::Quit));
    for junk in [
        "back w1",
        "visit",
        "visit x",
        "visit -1",
        "visit 1 2",
        "History",
    ] {
        assert_eq!(click(junk), None, "{junk:?}");
    }
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
fn what_is_typed_in_the_address_field_goes_to_an_address_or_a_search() {
    let went = |typed: &str| went(&format!("go {typed}"));
    assert_eq!(went("apple.com").as_deref(), Some("https://apple.com"));
    assert_eq!(
        went("https://habr.com/ru/post/210288/").as_deref(),
        Some("https://habr.com/ru/post/210288/")
    );
    assert_eq!(
        went("localhost:3000/x").as_deref(),
        Some("http://localhost:3000/x")
    );
    assert_eq!(
        went("rust borrow checker").as_deref(),
        Some("https://www.google.com/search?q=rust+borrow+checker")
    );
    assert_eq!(
        went("hotmail").as_deref(),
        Some("https://www.google.com/search?q=hotmail")
    );
    assert_eq!(
        went("a&b=c?").as_deref(),
        Some("https://www.google.com/search?q=a%26b%3Dc%3F")
    );
}

#[test]
fn only_web_addresses_are_gone_to_from_the_field() {
    assert_eq!(
        went("go javascript:alert(1)").as_deref(),
        Some("https://www.google.com/search?q=javascript%3Aalert%281%29")
    );
    assert_eq!(
        went("go file:///etc/passwd").as_deref(),
        Some("https://www.google.com/search?q=file%3A%2F%2F%2Fetc%2Fpasswd")
    );
    assert_eq!(went("go    "), None);
    assert_eq!(went("show w1"), None);
}

#[test]
fn the_toolbar_buttons_are_clicks() {
    assert_eq!(click("reload"), Some(Click::Reload));
    assert_eq!(click("new 812 40"), Some(Click::NewTab(812, 40)));
    assert_eq!(click("frame 760 40"), Some(Click::Frame(760, 40)));
    assert_eq!(click("frame 760"), None);
    let held = Tool::Mini.bit() | Tool::History.bit();
    assert_eq!(click("more 700 40 mn hs"), Some(Click::More(700, 40, held)));
    assert_eq!(
        click("more 700 40 fs"),
        None,
        "the page's tools are its toolbar's"
    );
    assert_eq!(click("more 700 40"), None, "nothing held");
    assert_eq!(
        click("more 700 40 hs xx"),
        None,
        "a button it does not know"
    );
    assert_eq!(click("new"), None);
    assert_eq!(click("new 812"), None);
}

#[test]
fn the_address_field_open_and_shut_are_clicks() {
    assert_eq!(click("typing on"), Some(Click::Typing(true)));
    assert_eq!(click("typing off"), Some(Click::Typing(false)));
    assert_eq!(click("typing maybe"), None);
}

#[test]
fn a_tabs_menu_is_asked_for_by_its_key_and_where() {
    assert_eq!(
        click("menu w3 120 40"),
        Some(Click::Menu(Key::Web(3), 120, 40))
    );
    assert_eq!(click("menu t1 0 7"), Some(Click::Menu(Key::Term(1), 0, 7)));
    assert_eq!(click("menu w3 120"), None);
    assert_eq!(click("menu x3 1 2"), None);
}

#[test]
fn where_the_strip_wants_glass_is_a_rectangle_or_none() {
    assert_eq!(
        click("glass 150 104 1240 212"),
        Some(Click::Glass(Some((150, 104, 1240, 212))))
    );
    assert_eq!(click("glass none"), Some(Click::Glass(None)));
    // The bar's crumbs: the project's and the tab's menus.
    assert_eq!(click("projects 92 34"), Some(Click::Projects(92, 34)));
    assert_eq!(click("tabs 210 34"), Some(Click::Tabs(210, 34)));
    assert_eq!(click("tabs 210"), None);
    assert_eq!(click("glass 1 2 3"), None);
    assert_eq!(click("glass 1 2 3 4 5"), None);
    assert_eq!(click("glass"), None);
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
    // The bar is a compact toolbar's height, as `BAR` says; the browser's toolbar `NAV`'s.
    assert!(PAGE.contains("height:38px") && (BAR - 38.0).abs() < f64::EPSILON);
    assert!(PAGE.contains("height:40px") && (NAV - 40.0).abs() < f64::EPSILON);
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
        script(&items, Key::Term(BOARD + 1), &[], None, &none())
            .contains(r#""kind":"board","title":"Board""#)
    );
}

#[test]
fn the_event_center_opens_closes_and_says_which_button_of_which_event() {
    assert_eq!(click("events"), Some(Click::Events));
    assert_eq!(click("unmenu"), Some(Click::Unmenu));
    assert_eq!(click("event 7 1"), Some(Click::Event(7, 1)));
    assert_eq!(click("clear"), Some(Click::Clear));
    for said in ["event", "event 7", "event x 1", "event 7 -1"] {
        assert_eq!(click(said), None, "{said}");
    }
    assert_eq!(center("", false), "V([], false)");
    assert_eq!(center(r#"[{"n":1}]"#, true), r#"V([{"n":1}], true)"#);
}

#[test]
fn the_side_says_what_the_strip_says_its_menus_placed_below_the_strip() {
    assert_eq!(click("side"), Some(Click::Side));
    assert_eq!(click("side-width 260"), Some(Click::SideWidth(260, false)));
    assert_eq!(
        click("side-width 300 done"),
        Some(Click::SideWidth(300, true))
    );
    assert_eq!(click("side-width 300 later"), None);
    assert_eq!(click("side-width -4"), None);
    assert_eq!(
        click("side-width usual done"),
        Some(Click::SideWidth(248, true))
    );
    // The side's points and the browser's toolbar's, said as the window's.
    assert_eq!(moved("menu t3 40 12", (0.0, 38.0)), "menu t3 40 50");
    assert_eq!(moved("new 200 30", (0.0, 84.4)), "new 200 114");
    assert_eq!(moved("show w2", (0.0, 38.0)), "show w2");
    assert_eq!(moved("menu t3 40 no", (0.0, 38.0)), "menu t3 40 no");
    assert_eq!(moved("frame 100 30", (264.0, 38.0)), "frame 364 68");
    assert_eq!(
        moved("glass 10 40 300 200", (264.0, 38.0)),
        "glass 274 78 300 200"
    );
    assert_eq!(moved("tabs 10 20", (5.0, 5.0)), "tabs 15 25");
    let drawn = script(
        &[item(Key::Web(1), "Docs", "proj")],
        Key::Web(1),
        &[],
        Some(false),
        &none(),
    );
    assert!(drawn.contains(r#""side":false"#), "{drawn}");
}

#[test]
fn a_terminal_running_an_agent_carries_its_mark_and_what_it_does() {
    let items = [Item {
        key: Key::Term(3),
        title: "session 3 · claude --resume".into(),
        detail: "Fix the cart button".into(),
        group: "proj".into(),
        agent: None,
    }];
    let script = script(&items, Key::Term(3), &[], Some(true), &none());
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
    }];
    let drawn = script(&items, Key::Term(4), &[], Some(true), &none());
    assert!(drawn.contains(r#""agent":"codex""#), "{drawn}");
    assert!(drawn.contains(r#""name":"Codex""#), "{drawn}");
}

#[test]
fn every_shortcut_is_a_click_the_strip_knows() {
    // A row of places does its click with the place after it (`key tab 3`).
    for shortcut in &crate::browser::shortcuts::SHORTCUTS {
        let does = match shortcut.keys.ends_with("+digit") {
            true => format!("{} 1", shortcut.does),
            false => shortcut.does.to_string(),
        };
        assert!(click(&does).is_some(), "{does}");
    }
    assert_eq!(click("key tab 3"), Some(Click::Key(Keyed::Tab(3))));
    assert_eq!(click("key tab 9"), None);
    assert_eq!(click("key tab last"), Some(Click::Key(Keyed::LastTab)));
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
fn a_dragged_tab_says_where_it_goes() {
    assert_eq!(
        click("move w2 t3"),
        Some(Click::Move(Key::Web(2), Some(Key::Term(3))))
    );
    assert_eq!(click("move t3 end"), Some(Click::Move(Key::Term(3), None)));
    assert_eq!(
        click("drop-over w1 400 120"),
        Some(Click::DropOver(Key::Web(1), 400, 120))
    );
    assert_eq!(click("drop-off"), Some(Click::DropOff));
    assert_eq!(
        click("drop w1 400 120"),
        Some(Click::Drop(Key::Web(1), 400, 120))
    );
    assert_eq!(
        click("split-move -40 done"),
        Some(Click::SplitMove(-40, true))
    );
    assert_eq!(click("move w2"), None);
    // The side's points are the window's once moved down by the bar.
    assert_eq!(moved("drop w1 400 120", (0.0, 38.0)), "drop w1 400 158");
    assert_eq!(
        moved("drop-over w1 400 120", (0.0, 38.0)),
        "drop-over w1 400 158"
    );
}
