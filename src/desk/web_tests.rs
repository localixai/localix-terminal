//! Tests of `web`.

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
}
