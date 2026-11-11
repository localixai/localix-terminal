//! UI message parsing and coordinate translation regressions.

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
fn the_toolbar_buttons_are_clicks() {
    assert_eq!(click("reload"), Some(Click::Reload));
    assert_eq!(click("new 812 40"), Some(Click::NewTab(812, 40)));
    assert_eq!(click("frame 760 40"), Some(Click::Frame(760, 40)));
    assert_eq!(click("frame 760"), None);
    let held = Tool::History.bit();
    assert_eq!(click("more 700 40 hs"), Some(Click::More(700, 40, held)));
    // The window is put away by its yellow button and ⌘M: the bar has no button of its own.
    assert_eq!(click("more 700 40 mn"), None);
    assert_eq!(
        click("more 700 40 fs"),
        None,
        "the page's tools are its toolbar's"
    );
    assert_eq!(click("more 700 40"), None, "nothing held");
    // The Event Center's button folds into "»" after history's, and is in its menu first.
    let both = Tool::Events.bit() | held;
    assert_eq!(click("more 700 40 ev hs"), Some(Click::More(700, 40, both)));
    assert_eq!(Tool::ALL, [Tool::Events, Tool::History]);
    assert_eq!(Tool::Events.label(), "Event Center");
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
    assert_eq!(click("glass 1 2 3 4 invalid"), None);
    assert_eq!(click("glass"), None);
}

#[test]
fn the_event_center_opens_closes_and_says_which_button_of_which_event() {
    assert_eq!(click("events"), Some(Click::Events));
    // A notice's click opens the list and never shuts it.
    assert_eq!(click("events open"), Some(Click::Listed));
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
        Some(false),
        &none(),
    );
    assert!(drawn.contains(r#""side":false"#), "{drawn}");
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
fn an_agents_mark_where_no_tab_is_left_asks_for_that_agent() {
    let first = agents::AGENTS[0].id;
    assert_eq!(click(&format!("agent {first}")), Some(Click::Agent(first)));
    // Only an agent the app knows: the page's word is never a command.
    assert_eq!(click("agent rm"), None);
    assert_eq!(click("agent"), None);
}
