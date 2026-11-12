//! A page that never came: what a tab waits for, what it keeps, and the line it shows.

use super::*;

const DEAD: &str = "http://localhost:5175/";

#[test]
fn an_address_asked_of_a_blank_tab_is_waited_for_until_its_page_comes() {
    let unloaded = Unloaded::default();
    assert!(unloaded.asked(1, DEAD, 5));
    assert!(unloaded.waits(1, 5));
    assert_eq!(unloaded.shows(1, DEAD), DEAD);
    // The page came: nothing waits, and nothing is left to fail.
    assert!(!unloaded.waits(1, 5));
    assert_eq!(unloaded.failed(1, 5), None);
    assert_eq!(unloaded.address(1), None);
}

#[test]
fn a_page_that_never_came_leaves_its_address_on_the_tab() {
    let unloaded = Unloaded::default();
    assert!(unloaded.asked(1, DEAD, 5));
    assert_eq!(unloaded.failed(1, 5).as_deref(), Some(DEAD));
    assert!(!unloaded.waits(1, 5));
    // The line is a blank document: the tab says the address it could not load.
    assert_eq!(unloaded.shows(1, BLANK), DEAD);
    assert_eq!(unloaded.address(1).as_deref(), Some(DEAD));
    // Asked again (Reload, another address), it is waited for as the first time.
    assert!(unloaded.asked(1, "http://b.test/", 9));
    assert_eq!(unloaded.shows(1, "http://b.test/"), "http://b.test/");
    assert_eq!(unloaded.address(1), None);
}

#[test]
fn what_a_page_asks_for_itself_is_its_own() {
    let unloaded = Unloaded::default();
    unloaded.shows(1, "http://a.test/");
    // A link of the page's, or a frame's address: WebKit keeps the page it shows.
    assert!(!unloaded.asked(1, "http://frame.test/", 7));
    assert!(!unloaded.waits(1, 7));
    // A blank document asked for is no page to wait for.
    assert!(!unloaded.asked(2, BLANK, 8));
    // The tab closed and its view blank again, what it loads next is waited for.
    unloaded.shows(1, BLANK);
    assert!(unloaded.asked(1, DEAD, 9));
}

#[test]
fn a_later_request_takes_the_place_of_the_one_waited_for() {
    let unloaded = Unloaded::default();
    assert!(unloaded.asked(1, DEAD, 5));
    assert!(unloaded.asked(1, "http://b.test/", 6));
    assert!(!unloaded.waits(1, 5));
    assert_eq!(unloaded.failed(1, 5), None);
    assert_eq!(unloaded.failed(1, 6).as_deref(), Some("http://b.test/"));
}

#[test]
fn a_tab_gone_keeps_nothing() {
    let unloaded = Unloaded::default();
    assert!(unloaded.asked(1, DEAD, 5));
    unloaded.failed(1, 5);
    unloaded.forget(1);
    assert_eq!(unloaded.address(1), None);
    assert_eq!(unloaded.shows(1, BLANK), BLANK);
}

#[test]
fn the_line_names_the_place_asked_for_and_takes_no_markup_from_it() {
    let line = |url: &str| {
        let unloaded = Unloaded::default();
        assert!(unloaded.asked(1, url, 5));
        unloaded.failed(1, 5).unwrap();
        unloaded.line(1).unwrap()
    };
    let page = line("http://localhost:5175/app?tab=<b>");
    assert!(page.contains("Could Not Load localhost:5175<"), "{page}");
    assert!(page.contains("<title>localhost:5175</title>"), "{page}");
    assert!(!page.contains("<b>"), "{page}");
    // A default port is not said, and what is no address is said as written, escaped.
    assert!(line("https://a.test/x").contains("Could Not Load a.test<"));
    assert_eq!(
        place("no address <b> & \"co\""),
        "no address &lt;b&gt; &amp; &quot;co&quot;"
    );
}

#[test]
fn a_tab_that_waits_is_asked_less_and_less_often() {
    assert!(pause(1) < pause(4));
    assert_eq!(pause(40), pause(400));
}

#[test]
fn a_reload_from_inside_the_tab_asks_for_the_address_it_keeps() {
    let unloaded = Unloaded::default();
    // A tab that shows its page reloads it.
    assert_eq!(unloaded.reloading(1), "location.reload()");
    assert!(unloaded.asked(1, DEAD, 5));
    assert_eq!(unloaded.failed(1, 5).as_deref(), Some(DEAD));
    // The line's own document is blank: reloading that would ask for nothing.
    assert_eq!(
        unloaded.reloading(1),
        r#"location.replace("http://localhost:5175/")"#
    );
    // An address is data in the script, never a part of it.
    assert!(unloaded.asked(2, "http://a.test/\");alert(1)//", 6));
    assert!(unloaded.failed(2, 6).is_some());
    assert_eq!(
        unloaded.reloading(2),
        r#"location.replace("http://a.test/\");alert(1)//")"#
    );
}

#[test]
fn what_came_and_no_page_shows_is_said_by_its_name() {
    let unloaded = Unloaded::default();
    let file = "http://127.0.0.1:5175/k3/build%20all.zip?x=1";
    assert!(unloaded.asked(1, file, 5));
    // It came as a file to keep: the tab is asked at once whether its view still loads.
    assert_eq!(unloaded.kept_apart(1), Some(5));
    assert_eq!(unloaded.failed(1, 5).as_deref(), Some(file));
    let line = unloaded.line(1).unwrap();
    assert!(line.contains(">No Preview for build all.zip<"), "{line}");
    assert!(line.contains("<title>build all.zip</title>"), "{line}");
    // The tab keeps the address, as for a page that never came.
    assert_eq!(unloaded.shows(1, BLANK), file);
    // Asked again, it is one more address that may come or not.
    assert!(unloaded.asked(1, DEAD, 6));
    assert_eq!(unloaded.failed(1, 6).as_deref(), Some(DEAD));
    let line = unloaded.line(1).unwrap();
    assert!(line.contains("Could Not Load localhost:5175<"), "{line}");
}

#[test]
fn a_file_a_page_keeps_apart_is_its_own() {
    let unloaded = Unloaded::default();
    // A page's link to an archive: nothing was waited for, and the page stays.
    unloaded.shows(1, "http://a.test/");
    assert_eq!(unloaded.kept_apart(1), None);
    assert_eq!(unloaded.line(1), None);
    // A tab nothing was asked of.
    assert_eq!(unloaded.kept_apart(2), None);
}

#[test]
fn a_file_no_page_shows_is_named_as_written_and_takes_no_markup() {
    assert_eq!(named("http://127.0.0.1:1/k/a%3Cb%3E.bin"), "a&lt;b&gt;.bin");
    assert_eq!(named("https://a.test/get/tool"), "tool");
    // An address with no name of a file is said as a place.
    assert_eq!(named("https://a.test/"), "a.test");
    assert_eq!(named("https://a.test:8443/get/"), "a.test:8443");
}
