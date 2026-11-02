//! Tests of `strip`.

use super::*;

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
            url: "https://a.example/\"><b>".into(),
            group: "proj".into(),
        },
        Item {
            key: Key::Term(2),
            title: "session 2 · python3".into(),
            url: String::new(),
            group: "proj".into(),
        },
    ];
    let visited = [Visited {
        title: "<i>Habr</i>".into(),
        url: "https://habr.com/ru/post/210288/".into(),
    }];
    let script = script(&items, Key::Term(2), &visited);
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
    assert_eq!(data["tabs"][0]["url"], "https://a.example/\"><b>");
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
        url: String::new(),
        group: String::new(),
    }];
    let script = script(&items, Key::Web(1), &[]);
    assert!(script.contains("\"title\":\"New tab\""), "{script}");
}

fn item(key: Key, title: &str, group: &str) -> Item {
    Item {
        key,
        title: title.into(),
        url: String::new(),
        group: group.into(),
    }
}

#[test]
fn one_project_draws_as_it_always_has() {
    let items = [
        item(Key::Web(1), "Docs", "proj"),
        item(Key::Term(2), "make", "proj"),
    ];
    assert_eq!(
        script(&items, Key::Web(1), &[]),
        r#"S({"shown":"w1","tabs":[{"key":"w1","kind":"web","title":"Docs","url":""},{"key":"t2","kind":"term","title":"make","url":""}],"visited":[]})"#
    );
}

#[test]
fn several_projects_draw_their_tabs_together_each_saying_its_project() {
    let items = [
        item(Key::Web(1), "Docs", "proj"),
        item(Key::Term(2), "make", "<b>shop</b>"),
        item(Key::Web(3), "Issues", "proj"),
    ];
    let script = script(&items, Key::Web(1), &[]);
    let json = script
        .strip_prefix("S(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    let tabs = data["tabs"].as_array().unwrap();
    let keys: Vec<&str> = tabs
        .iter()
        .map(|tab| tab["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["w1", "w3", "t2"]);
    let labels: Vec<&serde_json::Value> = tabs.iter().map(|tab| &tab["group"]).collect();
    assert_eq!(
        labels,
        [
            &serde_json::json!("proj"),
            &serde_json::json!("proj"),
            &serde_json::json!("<b>shop</b>")
        ]
    );
    // A project's name is set as text too.
    assert!(PAGE.contains("label.textContent = name"));
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
    let row = |at, key, state| Row {
        at,
        key,
        title: "\"</script><b>x</b>".into(),
        detail: "https://a.example/".into(),
        state,
        group: "proj".into(),
    };
    let rows = [
        row(7, Key::Term(2), State::Hidden),
        row(3, Key::Web(1), State::Closed),
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
    assert_eq!(click("glass 1 2 3"), None);
    assert_eq!(click("glass 1 2 3 4 5"), None);
    assert_eq!(click("glass"), None);
}

#[test]
fn the_toolbar_sits_in_the_titlebar_and_the_projects_have_a_row_of_their_own_above_the_tabs() {
    let at = |id: &str| PAGE.find(&format!("id=\"{id}\"")).unwrap();
    // Top to bottom: the toolbar (beside the window's buttons), the projects, the tabs.
    assert!(at("bar") < at("projects") && at("projects") < at("tabs"));
    // Only the toolbar is inset for the window's buttons; the projects run from the edge.
    assert!(PAGE.contains("html[data-glass] #bar{padding-left:92px}"));
    assert!(!PAGE.contains("#projects.on,html[data-glass]"));
}

#[test]
fn every_row_of_the_strip_is_inked_for_the_page_under_it() {
    // A light page under a strip in the dark appearance: a row left in the system's colour is
    // light words on a light bar.
    assert!(PAGE.contains(
        "html[data-tint] #projects,html[data-tint] #bar,html[data-tint] #tabs{color:var(--ink)"
    ));
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
