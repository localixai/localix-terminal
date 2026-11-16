//! Tests of find in what the window shows: which tab finds, and what a page tab's row keeps.

use super::*;

#[test]
fn a_live_terminal_finds_in_its_page_and_a_page_through_the_toolbar() {
    let live = Some((Held::Live, "term"));
    assert_eq!(finder(Shown::Term(2), live, false), Some(Finder::Panel(2)));
    assert_eq!(finder(Shown::Web, None, true), Some(Finder::Page));
}

#[test]
fn nothing_else_finds() {
    // A board or Settings (their own kinds), a terminal closed or ended, one not in the window,
    // a page with no toolbar over it (a headless window's), no tab at all.
    let board = Some((Held::Live, "board"));
    assert_eq!(finder(Shown::Term(1 << 24), board, true), None);
    assert_eq!(
        finder(Shown::Term(2), Some((Held::Ended, "term")), true),
        None
    );
    assert_eq!(
        finder(Shown::Term(2), Some((Held::Hidden, "term")), true),
        None
    );
    assert_eq!(finder(Shown::Term(2), None, true), None);
    assert_eq!(finder(Shown::Web, None, false), None);
    assert_eq!(finder(Shown::Nothing, None, true), None);
}

fn found(query: &str, at: i64, of: u32) -> Found {
    Found {
        tab: 3,
        query: query.into(),
        at,
        of,
    }
}

#[test]
fn a_row_counts_what_its_page_found_of_the_query_it_holds_now() {
    let mut sought = Sought::default();
    sought.asked("nee", "#0a84ff");
    sought.asked("needle", "#0a84ff");
    // The count of a query typed over comes late: it is not this one's.
    assert!(!sought.found(&found("nee", 0, 9)));
    assert_eq!(sought.told(3)["of"], Value::Null);
    assert!(sought.found(&found("needle", 2, 12)));
    assert_eq!(
        sought.told(3),
        json!({ "tab": 3, "query": "needle", "at": 2, "of": 12 })
    );
    // The same query again (a step, another accent) keeps its count until the page says more;
    // another query has none yet.
    sought.asked("needle", "#8e8e93");
    assert_eq!(sought.told(3)["of"], 12);
    sought.asked("needles", "#8e8e93");
    assert_eq!(sought.told(3)["at"], Value::Null);
}

#[test]
fn the_toolbar_grows_by_the_find_rows_own_height() {
    let sheet = include_str!("frame.css");
    assert!(sheet.contains(&format!(":where(.finds){{height:{FIND_ROW}px;")));
}
