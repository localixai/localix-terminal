//! Tests of `web`.

use super::controls::control;
use super::reading::{TITLE_LIMIT, clipped, short_refs, text_end};
use super::*;

#[test]
fn an_address_without_a_scheme_is_https_but_this_machine_is_http() {
    assert_eq!(
        web_address("example.com/a").unwrap(),
        "https://example.com/a"
    );
    assert_eq!(
        web_address("localhost:3000").unwrap(),
        "http://localhost:3000"
    );
    assert_eq!(
        web_address("127.0.0.1:80/x").unwrap(),
        "http://127.0.0.1:80/x"
    );
}

#[test]
fn only_the_web_opens_in_the_window() {
    assert!(
        web_address("file:///etc/passwd")
            .unwrap_err()
            .contains("only http")
    );
    assert!(web_address("javascript:alert(1)").is_err());
    assert!(web_address("http://a b").is_err());
}

#[test]
fn each_control_shows_its_ref_shortly_and_a_call_names_it_any_way() {
    assert_eq!(
        short_refs("- button \"Go\" [ref=e1]\n- link [ref=e2]: x"),
        "- button \"Go\" [e1]\n- link [e2]: x"
    );
    for given in ["e12", "@e12", "[e12]", " e12 "] {
        assert_eq!(control(given).unwrap(), "e12");
    }
    for wrong in ["12", "e", "button", "e1x"] {
        assert!(control(wrong).is_err(), "{wrong}");
    }
    // The toolbar's controls are in the outline with their refs: a call may name them.
    for given in ["back", "@reload", "[back]"] {
        assert!(
            ["back", "reload"].contains(&control(given).unwrap().as_str()),
            "{given}"
        );
    }
}

#[test]
fn a_pages_title_is_cut_where_it_is_too_long() {
    assert_eq!(clipped("short", 200), "short");
    assert_eq!(clipped("héllo wörld", 5), "héllo…");
    let long = "A".repeat(10_000_000);
    assert_eq!(clipped(&long, TITLE_LIMIT).chars().count(), TITLE_LIMIT + 1);
}

#[test]
fn a_window_of_text_ends_in_the_probes_units() {
    assert_eq!(text_end(10, "abc"), 13);
    assert_eq!(
        text_end(0, "a😀b"),
        4,
        "an emoji is two units, as the page counts"
    );
    assert_eq!(text_end(5, ""), 5);
}
